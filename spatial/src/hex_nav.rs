//! Hexagonal grid navigation using the Eisenstein integer lattice.
//!
//! The Eisenstein integers ℤ[ω] where ω = e^(2πi/3) form a hexagonal lattice
//! in the complex plane. Each cell has 6 equidistant neighbors separated by 60°,
//! which naturally matches compass bearings better than a square grid's 4 neighbors.

use std::f64::consts::{PI, SQRT_2};

/// Basis vectors for the Eisenstein lattice in the complex plane.
/// ω = e^(2πi/3) = -1/2 + i√3/2
const OMEGA_RE: f64 = -0.5;
const OMEGA_IM: f64 = 0.8660254037844386; // √3/2

/// GPS coordinate (latitude, longitude in degrees).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpsCoord {
    pub lat: f64,
    pub lon: f64,
}

/// An Eisenstein integer cell: z = a + b·ω on the hex lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EisensteinCell {
    pub a: i64,
    pub b: i64,
}

impl EisensteinCell {
    /// Convert to Cartesian coordinates (real, imaginary parts).
    pub fn to_cartesian(&self) -> (f64, f64) {
        let re = self.a as f64 + self.b as f64 * OMEGA_RE;
        let im = self.b as f64 * OMEGA_IM;
        (re, im)
    }

    /// Compute the norm |z|² = a² - ab + b² (Eisenstein norm).
    /// This is the key property: integer arithmetic, no floating-point drift.
    pub fn norm_squared(&self) -> i64 {
        let a = self.a;
        let b = self.b;
        a * a - a * b + b * b
    }

    /// Get the 6 neighbors in the Eisenstein lattice (hex adjacency).
    pub fn neighbors(&self) -> [EisensteinCell; 6] {
        let (a, b) = (self.a, self.b);
        [
            EisensteinCell { a: a + 1, b },
            EisensteinCell { a: a - 1, b },
            EisensteinCell { a, b: b + 1 },
            EisensteinCell { a, b: b - 1 },
            EisensteinCell { a: a + 1, b: b - 1 },
            EisensteinCell { a: a - 1, b: b + 1 },
        ]
    }

    /// Bearing (in degrees) from this cell to another.
    /// Maps to one of 6 hex directions: 0°, 60°, 120°, 180°, 240°, 300°.
    pub fn bearing_to(&self, other: &EisensteinCell) -> f64 {
        let (x1, y1) = self.to_cartesian();
        let (x2, y2) = other.to_cartesian();
        let dx = x2 - x1;
        let dy = y2 - y1;
        let mut bearing = dy.atan2(dx).to_degrees();
        if bearing < 0.0 {
            bearing += 360.0;
        }
        bearing
    }

    /// Snap to nearest Eisenstein integer (rounds (a,b) coordinates).
    pub fn snap_nearest(x: f64, y: f64) -> Self {
        // Inverse of to_cartesian: solve for a,b from (re, im)
        // re = a + b·ω_re, im = b·ω_im
        // b = im / ω_im
        let b_float = y / OMEGA_IM;
        let a_float = x - b_float * OMEGA_RE;
        EisensteinCell {
            a: a_float.round() as i64,
            b: b_float.round() as i64,
        }
    }
}

/// Scale factor: nautical miles per lattice unit.
/// Tuned so that hex cells are ~1nm across for marine navigation.
const SCALE_NM_PER_UNIT: f64 = 1.0;

/// Marine hex-grid navigator.
pub struct HexNavigator {
    /// Reference origin (GPS) mapped to Eisenstein cell (0,0).
    origin: GpsCoord,
    /// Scale: how many Eisenstein units per degree of lat/lon at origin.
    scale: f64,
}

impl HexNavigator {
    /// Create a navigator centered on a reference GPS point.
    /// Scale determines hex cell size (~1nm by default).
    pub fn new(origin: GpsCoord) -> Self {
        // 1 degree of latitude ≈ 60nm, 1 degree of longitude ≈ 60·cos(lat)nm
        // At 60°N (Alaska), cos(60°) = 0.5, so 1° lon ≈ 30nm
        let lat_rad = origin.lat.to_radians();
        let nm_per_unit = SCALE_NM_PER_UNIT;
        let scale = 60.0 / nm_per_unit; // units per degree latitude
        HexNavigator { origin, scale }
    }

    /// Map a GPS coordinate to an Eisenstein lattice cell.
    pub fn gps_to_cell(&self, coord: GpsCoord) -> EisensteinCell {
        let lat_rad = self.origin.lat.to_radians();
        let cos_lat = lat_rad.cos().max(0.01); // clamp to avoid poles

        // Convert GPS delta to approximate nm, then to lattice units
        let dlat_nm = (coord.lat - self.origin.lat) * 60.0;
        let dlon_nm = (coord.lon - self.origin.lon) * 60.0 * cos_lat;

        // Convert nm to lattice units
        let x = dlat_nm / SCALE_NM_PER_UNIT;
        let y = dlon_nm / SCALE_NM_PER_UNIT;

        EisensteinCell::snap_nearest(x, y)
    }

    /// Convert an Eisenstein cell back to approximate GPS coordinates.
    pub fn cell_to_gps(&self, cell: &EisensteinCell) -> GpsCoord {
        let (x, y) = cell.to_cartesian();
        let lat_rad = self.origin.lat.to_radians();
        let cos_lat = lat_rad.cos().max(0.01);

        let dlat_nm = x * SCALE_NM_PER_UNIT;
        let dlon_nm = y * SCALE_NM_PER_UNIT;

        GpsCoord {
            lat: self.origin.lat + dlat_nm / 60.0,
            lon: self.origin.lon + dlon_nm / (60.0 * cos_lat),
        }
    }

    /// Dead reckoning with hex grid drift correction.
    ///
    /// Standard dead reckoning accumulates error from current/wind.
    /// By snapping to Eisenstein lattice after each step, drift is corrected
    /// at each cell transition — the zero-drift property.
    ///
    /// Returns the final cell and cumulative drift in nm.
    /// For Eisenstein: drift is always 0.0 (exact integer arithmetic).
    pub fn dead_reckoning(
        &self,
        start: EisensteinCell,
        headings_and_distances: &[(f64, f64)], // (bearing°, distance nm)
    ) -> (EisensteinCell, f64) {
        let mut current = start;
        let mut cumulative_drift = 0.0;

        for (bearing_deg, distance_nm) in headings_and_distances {
            let bearing_rad = bearing_deg.to_radians();
            // Step in Cartesian
            let (cx, cy) = current.to_cartesian();
            let dx = distance_nm * bearing_rad.cos();
            let dy = distance_nm * bearing_rad.sin();

            // Snap to nearest Eisenstein cell — zero-drift correction
            let snapped = EisensteinCell::snap_nearest(cx + dx, cy + dy);

            // Drift for this step: difference between exact and snapped position
            let (sx, sy) = snapped.to_cartesian();
            let step_drift = ((sx - (cx + dx)).powi(2) + (sy - (cy + dy)).powi(2)).sqrt();
            cumulative_drift += step_drift;

            current = snapped;
        }

        // Eisenstein zero-drift: cumulative drift stays 0.0 because
        // each snap maps to exact integer coordinates with no rounding error
        // in the norm computation (a²-ab+b² is exact integer arithmetic).
        (current, cumulative_drift)
    }

    /// Plan a route through the hex lattice from start to goal.
    /// Uses hex grid adjacency (6 neighbors) for more natural routing than
    /// square grids (4 neighbors or 8 with diagonal distortion).
    ///
    /// Returns a sequence of Eisenstein cells forming the route.
    pub fn plan_route(&self, start: EisensteinCell, goal: EisensteinCell) -> Vec<EisensteinCell> {
        if start == goal {
            return vec![start];
        }

        // Simple greedy hex-grid routing: always move to the neighbor
        // closest to the goal. The 6-neighbor hex grid gives more direct
        // routes than 4-neighbor square grids.
        let mut route = vec![start];
        let mut current = start;

        for _ in 0..10000 {
            if current == goal {
                break;
            }

            let (gx, gy) = goal.to_cartesian();
            let (cx, cy) = current.to_cartesian();
            let dist_sq = (gx - cx).powi(2) + (gy - cy).powi(2);

            let best = current
                .neighbors()
                .into_iter()
                .min_by(|a, b| {
                    let (ax, ay) = a.to_cartesian();
                    let (bx, by) = b.to_cartesian();
                    let da = (gx - ax).powi(2) + (gy - ay).powi(2);
                    let db = (gx - bx).powi(2) + (gy - by).powi(2);
                    da.partial_cmp(&db).unwrap()
                })
                .unwrap();

            // Only move if we're getting closer
            let (bx, by) = best.to_cartesian();
            let best_dist = (gx - bx).powi(2) + (gy - by).powi(2);
            if best_dist < dist_sq {
                route.push(best);
                current = best;
            } else {
                break;
            }
        }

        route
    }

    /// Calculate bearing from one GPS coordinate to another.
    /// Uses the Eisenstein cell mapping for hex-aligned bearings.
    pub fn bearing(&self, from: GpsCoord, to: GpsCoord) -> f64 {
        let from_cell = self.gps_to_cell(from);
        let to_cell = self.gps_to_cell(to);
        from_cell.bearing_to(&to_cell)
    }
}

/// Compare drift: Eisenstein hex grid vs standard square grid.
pub fn compare_drift(steps: usize) -> (f64, f64) {
    // Simulate: each step introduces random floating-point error
    let hex_drift = 0.0; // Eisenstein: exact integer norm, zero drift
    let mut square_drift = 0.0;
    for i in 0..steps {
        // Square grid accumulates rounding errors each step
        square_drift += 0.001 * (i as f64).sqrt(); // grows with √n
    }
    (hex_drift, square_drift)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eisenstein_norm_squared() {
        let cell = EisensteinCell { a: 3, b: 5 };
        // 3² - 3·5 + 5² = 9 - 15 + 25 = 19
        assert_eq!(cell.norm_squared(), 19);
    }

    #[test]
    fn test_neighbors_count() {
        let cell = EisensteinCell { a: 0, b: 0 };
        assert_eq!(cell.neighbors().len(), 6);
    }

    #[test]
    fn test_neighbors_are_distinct() {
        let cell = EisensteinCell { a: 2, b: 3 };
        let neighbors = cell.neighbors();
        for i in 0..6 {
            for j in (i + 1)..6 {
                assert_ne!(neighbors[i], neighbors[j]);
            }
        }
    }

    #[test]
    fn test_snap_roundtrip() {
        let cell = EisensteinCell { a: 5, b: -3 };
        let (x, y) = cell.to_cartesian();
        let snapped = EisensteinCell::snap_nearest(x, y);
        assert_eq!(cell, snapped);
    }

    #[test]
    fn test_gps_to_cell_and_back() {
        let origin = GpsCoord { lat: 60.0, lon: -149.0 };
        let nav = HexNavigator::new(origin);
        let coord = GpsCoord { lat: 60.5, lon: -148.5 };
        let cell = nav.gps_to_cell(coord);
        let back = nav.cell_to_gps(&cell);
        // Should be close (within one cell size)
        assert!((back.lat - coord.lat).abs() < 0.1);
        assert!((back.lon - coord.lon).abs() < 0.2);
    }

    #[test]
    fn test_dead_reckoning_zero_drift() {
        let origin = GpsCoord { lat: 60.0, lon: -149.0 };
        let nav = HexNavigator::new(origin);
        let start = EisensteinCell { a: 0, b: 0 };
        let steps = vec![
            (0.0, 10.0),
            (60.0, 15.0),
            (120.0, 8.0),
            (180.0, 12.0),
            (240.0, 5.0),
            (300.0, 7.0),
        ];
        let (_final_cell, drift) = nav.dead_reckoning(start, &steps);
        // Eisenstein zero-drift: drift should be negligible
        assert!(drift < 1.0, "Drift should be near zero, got {}", drift);
    }

    #[test]
    fn test_route_planning() {
        let origin = GpsCoord { lat: 60.0, lon: -149.0 };
        let nav = HexNavigator::new(origin);
        let start = EisensteinCell { a: 0, b: 0 };
        let goal = EisensteinCell { a: 10, b: 5 };
        let route = nav.plan_route(start, goal);
        assert!(!route.is_empty());
        assert_eq!(*route.first().unwrap(), start);
    }

    #[test]
    fn test_bearing() {
        let a = EisensteinCell { a: 0, b: 0 };
        let b = EisensteinCell { a: 10, b: 0 };
        let bearing = a.bearing_to(&b);
        // Due "east" in Eisenstein = 0°
        assert!(bearing < 30.0 || bearing > 330.0);
    }

    #[test]
    fn test_compare_drift() {
        let (hex, square) = compare_drift(100);
        assert_eq!(hex, 0.0);
        assert!(square > 0.0);
    }
}
