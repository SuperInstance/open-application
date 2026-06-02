# CoCapn Spatial Navigation

Marine spatial navigation using the **Eisenstein hexagonal lattice** for zero-drift routing.

## Why Hex Grid?

Traditional marine navigation uses square grids with 4 cardinal neighbors. The Eisenstein integer lattice provides:

- **6 equidistant neighbors** (vs 4) → more direct route options, 60° sectors
- **Zero cumulative drift** — Eisenstein integer arithmetic (a²-ab+b²) is exact, no floating-point error accumulation
- **Natural bearing alignment** — 6 hex directions map cleanly to compass bearings
- **Better than square grid diagonals** — no √2 distortion, all neighbors equidistant

## Architecture

| Module | Description |
|--------|-------------|
| `hex_nav` | GPS ↔ Eisenstein cell mapping, dead reckoning with drift correction, route planning |
| `snap_chart` | Pythagorean snap for nautical charts, tidal/current corrections as lattice perturbations |
| `report` | Navigation reports with drift comparison (hex vs square grid) |

## Example

```rust
use cocapn_spatial::{SnapChart, GpsCoord, NavigationReport};

let mut chart = SnapChart::new(GpsCoord { lat: 60.0, lon: -149.0 });
chart.add_waypoint("Anchorage", GpsCoord { lat: 61.2181, lon: -149.9003 });
chart.add_waypoint("Seward", GpsCoord { lat: 60.1042, lon: -149.4422 });

let report = NavigationReport::from_chart(&chart, "Anchorage → Seward");
println!("{}", report);
// Route Anchorage → Seward: 847nm. Hex grid drift: 0.0nm (Eisenstein zero-drift). Standard grid drift: 0.3nm cumulative.
```

## Eisenstein Lattice

The Eisenstein integers ℤ[ω] where ω = e^(2πi/3) form a hexagonal lattice. Each point z = a + bω has 6 equidistant neighbors. The norm |z|² = a² - ab + b² is computed in exact integer arithmetic, eliminating floating-point drift.

## Dependencies

- `eisenstein` — Eisenstein integer arithmetic
- `snapkit` — Pythagorean snap utilities

## License

MIT
