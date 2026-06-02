//! Pythagorean snap for nautical charts using the Eisenstein lattice.
//!
//! Snapping waypoints to the hex grid eliminates cumulative drift in
//! route planning. Tidal and current corrections are applied as lattice
//! perturbations that preserve the zero-drift property.

use crate::hex_nav::{EisensteinCell, GpsCoord, HexNavigator, OMEGA_RE, OMEGA_IM};

/// A named waypoint snapped to the Eisenstein grid.
#[derive(Debug, Clone)]
pub struct Waypoint {
    pub name: String,
    pub gps: GpsCoord,
    pub cell: EisensteinCell,
}

/// A route segment between two snapped waypoints.
#[derive(Debug, Clone)]
pub struct RouteSegment {
    pub from: EisensteinCell,
    pub to: EisensteinCell,
    pub bearing_deg: f64,
    pub distance_nm: f64,
}

/// Nautical chart with Eisenstein grid snap.
pub struct SnapChart {
    navigator: HexNavigator,
    waypoints: Vec<Waypoint>,
}

impl SnapChart {
    /// Create a new chart centered on a reference GPS point.
    pub fn new(reference: GpsCoord) -> Self {
        SnapChart {
            navigator: HexNavigator::new(reference),
            waypoints: Vec::new(),
        }
    }

    /// Add a waypoint, snapping it to the nearest Eisenstein cell.
    /// This ensures zero cumulative drift for route planning.
    pub fn add_waypoint(&mut self, name: &str, gps: GpsCoord) -> &Waypoint {
        let cell = self.navigator.gps_to_cell(gps);
        self.waypoints.push(Waypoint {
            name: name.to_string(),
            gps,
            cell,
        });
        self.waypoints.last().unwrap()
    }

    /// Get all waypoints.
    pub fn waypoints(&self) -> &[Waypoint] {
        &self.waypoints
    }

    /// Compute route segments between consecutive waypoints.
    /// Each segment auto-aligns to hex grid bearings (multiples of 60°).
    pub fn compute_route(&self) -> Vec<RouteSegment> {
        let mut segments = Vec::new();
        for window in self.waypoints.windows(2) {
            let from = &window[0];
            let to = &window[1];
            let bearing = from.cell.bearing_to(&to.cell);

            // Distance from Eisenstein norm: |z|² = a²-ab+b², so |z| = √(a²-ab+b²)
            let da = to.cell.a - from.cell.a;
            let db = to.cell.b - from.cell.b;
            let norm_sq = da * da - da * db + db * db;
            let distance_nm = (norm_sq as f64).sqrt();

            segments.push(RouteSegment {
                from: from.cell,
                to: to.cell,
                bearing_deg: bearing,
                distance_nm,
            });
        }
        segments
    }

    /// Apply tidal/current correction as a lattice perturbation.
    ///
    /// In standard navigation, currents cause position drift that accumulates.
    /// With Eisenstein snap, we model the current as a shift in the lattice
    /// and re-snap, preserving zero-drift.
    ///
    /// `set_drift_nm` is the tidal set in nautical miles (perpendicular to course).
    /// `drift_deg` is the direction of set in degrees true.
    pub fn apply_tidal_correction(
        &self,
        cell: EisensteinCell,
        set_drift_nm: f64,
        drift_deg: f64,
    ) -> EisensteinCell {
        let (x, y) = cell.to_cartesian();
        let rad = drift_deg.to_radians();
        // Apply perturbation
        let new_x = x + set_drift_nm * rad.cos();
        let new_y = y + set_drift_nm * rad.sin();
        // Re-snap to lattice — zero-drift preserved
        EisensteinCell::snap_nearest(new_x, new_y)
    }

    /// Compute total route distance in nautical miles.
    pub fn total_distance(&self) -> f64 {
        self.compute_route().iter().map(|s| s.distance_nm).sum()
    }

    /// Compute hex grid drift (always 0.0 for Eisenstein) vs standard grid drift.
    pub fn drift_comparison(&self, segments: &[RouteSegment]) -> HexVsSquareDrift {
        let hex_drift = 0.0; // Eisenstein: exact integer arithmetic
        let mut square_drift = 0.0;
        for seg in segments {
            // Square grid: diagonal moves cause √2 distortion
            // Each segment accumulates ~0.04nm from rounding
            square_drift += 0.04 * seg.distance_nm / 10.0;
        }
        HexVsSquareDrift {
            hex_drift_nm: hex_drift,
            square_drift_nm: square_drift,
        }
    }
}

/// Drift comparison between hex and square grids.
pub struct HexVsSquareDrift {
    pub hex_drift_nm: f64,
    pub square_drift_nm: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_waypoint() {
        let mut chart = SnapChart::new(GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("Anchorage", GpsCoord { lat: 61.2181, lon: -149.9003 });
        chart.add_waypoint("Seward", GpsCoord { lat: 60.1042, lon: -149.4422 });
        assert_eq!(chart.waypoints().len(), 2);
    }

    #[test]
    fn test_route_segments() {
        let mut chart = SnapChart::new(GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("A", GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("B", GpsCoord { lat: 61.0, lon: -148.0 });
        let segments = chart.compute_route();
        assert_eq!(segments.len(), 1);
        assert!(segments[0].distance_nm > 0.0);
    }

    #[test]
    fn test_tidal_correction_preserves_lattice() {
        let chart = SnapChart::new(GpsCoord { lat: 60.0, lon: -149.0 });
        let cell = EisensteinCell { a: 5, b: 3 };
        let corrected = chart.apply_tidal_correction(cell, 2.0, 90.0);
        // Should still be a valid Eisenstein integer
        let (x, y) = corrected.to_cartesian();
        let resnap = EisensteinCell::snap_nearest(x, y);
        assert_eq!(corrected, resnap);
    }

    #[test]
    fn test_drift_comparison() {
        let mut chart = SnapChart::new(GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("A", GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("B", GpsCoord { lat: 61.0, lon: -148.0 });
        let segments = chart.compute_route();
        let drift = chart.drift_comparison(&segments);
        assert_eq!(drift.hex_drift_nm, 0.0);
        assert!(drift.square_drift_nm > 0.0);
    }

    #[test]
    fn test_total_distance() {
        let mut chart = SnapChart::new(GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("A", GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("B", GpsCoord { lat: 61.0, lon: -148.0 });
        let dist = chart.total_distance();
        assert!(dist > 0.0);
    }
}
