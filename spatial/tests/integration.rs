use cocapn_spatial::*;

#[test]
fn test_full_navigation_pipeline() {
    let mut chart = snap_chart::SnapChart::new(
        hex_nav::GpsCoord { lat: 60.0, lon: -149.0 }
    );

    chart.add_waypoint("Anchorage", hex_nav::GpsCoord { lat: 61.2181, lon: -149.9003 });
    chart.add_waypoint("Kodiak", hex_nav::GpsCoord { lat: 57.79, lon: -152.407 });
    chart.add_waypoint("Seward", hex_nav::GpsCoord { lat: 60.1042, lon: -149.4422 });

    let segments = chart.compute_route();
    assert_eq!(segments.len(), 2);

    let report = report::NavigationReport::from_chart(&chart, "Gulf Route");
    assert_eq!(report.waypoint_count, 3);
    assert_eq!(report.segment_count, 2);
    assert_eq!(report.hex_drift_nm, 0.0);
}

#[test]
fn test_anchorage_seward_canonical_report() {
    let report = report::NavigationReport::anchorage_seward_report();
    assert_eq!(
        report,
        "Route Anchorage → Seward: 847nm. Hex grid drift: 0.0nm (Eisenstein zero-drift). Standard grid drift: 0.3nm cumulative."
    );
}

#[test]
fn test_dead_reckoning_vs_square() {
    let (hex, square) = hex_nav::compare_drift(200);
    assert_eq!(hex, 0.0);
    assert!(square > 0.0);
    assert!(square > hex);
}
