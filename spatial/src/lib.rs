//! # CoCapn Spatial Navigation
//!
//! Marine spatial navigation using the Eisenstein hexagonal lattice.
//!
//! The hex grid provides natural advantages for marine navigation:
//! - 6 equidistant neighbors (vs 4 for square grids) → more route options
//! - Zero cumulative drift via Eisenstein integer arithmetic
//! - Bearing calculation maps naturally to 60° sectors
//!
//! ## Architecture
//!
//! - [`hex_nav`] — GPS ↔ Eisenstein lattice mapping, dead reckoning, route planning
//! - [`snap_chart`] — Pythagorean snap for nautical charts, tidal/current correction
//! - [`report`] — Navigation reports with drift statistics

pub mod hex_nav;
pub mod snap_chart;
pub mod report;

pub use hex_nav::{HexNavigator, GpsCoord, EisensteinCell};
pub use snap_chart::{SnapChart, Waypoint, RouteSegment};
pub use report::NavigationReport;
