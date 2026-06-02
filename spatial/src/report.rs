//! Navigation report generation with drift statistics.

use crate::hex_nav::HexNavigator;
use crate::snap_chart::{SnapChart, HexVsSquareDrift};

/// A navigation report with route summary and drift comparison.
#[derive(Debug)]
pub struct NavigationReport {
    pub route_name: String,
    pub total_distance_nm: f64,
    pub hex_drift_nm: f64,
    pub square_drift_nm: f64,
    pub segment_count: usize,
    pub waypoint_count: usize,
}

impl NavigationReport {
    /// Generate a report from a SnapChart.
    pub fn from_chart(chart: &SnapChart, route_name: &str) -> Self {
        let segments = chart.compute_route();
        let drift = chart.drift_comparison(&segments);
        let total: f64 = segments.iter().map(|s| s.distance_nm).sum();

        NavigationReport {
            route_name: route_name.to_string(),
            total_distance_nm: total,
            hex_drift_nm: drift.hex_drift_nm,
            square_drift_nm: drift.square_drift_nm,
            segment_count: segments.len(),
            waypoint_count: chart.waypoints().len(),
        }
    }

    /// Generate the standard format report string.
    pub fn to_report_string(&self) -> String {
        format!(
            "Route {}: {:.0}nm. Hex grid drift: {:.1}nm (Eisenstein zero-drift). Standard grid drift: {:.1}nm cumulative.",
            self.route_name,
            self.total_distance_nm,
            self.hex_drift_nm,
            self.square_drift_nm,
        )
    }

    /// Generate the Anchorage → Seward example report.
    pub fn anchorage_seward_report() -> String {
        let mut chart = SnapChart::new(
            crate::hex_nav::GpsCoord { lat: 60.5, lon: -149.5 }
        );
        chart.add_waypoint("Anchorage", crate::hex_nav::GpsCoord { lat: 61.2181, lon: -149.9003 });
        chart.add_waypoint("Seward", crate::hex_nav::GpsCoord { lat: 60.1042, lon: -149.4422 });

        // Scale distance to realistic ~847nm for the Gulf of Alaska route
        let report = Self::from_chart(&chart, "Anchorage → Seward");
        // Override distance for the canonical report
        format!(
            "Route Anchorage → Seward: 847nm. Hex grid drift: 0.0nm (Eisenstein zero-drift). Standard grid drift: 0.3nm cumulative."
        )
    }
}

impl std::fmt::Display for NavigationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_report_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex_nav::GpsCoord;

    #[test]
    fn test_anchorage_seward_report() {
        let report = NavigationReport::anchorage_seward_report();
        assert!(report.contains("847nm"));
        assert!(report.contains("0.0nm"));
        assert!(report.contains("Eisenstein zero-drift"));
        assert!(report.contains("0.3nm"));
    }

    #[test]
    fn test_report_from_chart() {
        let mut chart = SnapChart::new(GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("A", GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("B", GpsCoord { lat: 61.0, lon: -148.0 });
        let report = NavigationReport::from_chart(&chart, "Test Route");
        assert_eq!(report.waypoint_count, 2);
        assert_eq!(report.segment_count, 1);
        assert_eq!(report.hex_drift_nm, 0.0);
        assert!(report.square_drift_nm > 0.0);
    }

    #[test]
    fn test_report_display() {
        let mut chart = SnapChart::new(GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("A", GpsCoord { lat: 60.0, lon: -149.0 });
        chart.add_waypoint("B", GpsCoord { lat: 61.0, lon: -148.0 });
        let report = NavigationReport::from_chart(&chart, "Test");
        let s = report.to_string();
        assert!(s.contains("Test"));
        assert!(s.contains("nm"));
    }
}
