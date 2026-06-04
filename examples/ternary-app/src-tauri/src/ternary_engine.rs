// ─── Ternary App — Tauri Integration ────────────────────────────────────────
//
// Demonstrates ternary intelligence running as a desktop app via Tauri.
// The same engine can run as WASM (BrowserConstruct) or native (TuiConstruct).

/// A ternary cell in the spreadsheet grid
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellValue {
    Avoid,   // -1 — avoid/empty
    Unknown, //  0 — unexplored
    Choose,  // +1 — chosen/filled
}

impl CellValue {
    pub fn value(&self) -> i8 {
        match self {
            CellValue::Avoid => -1,
            CellValue::Unknown => 0,
            CellValue::Choose => 1,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            CellValue::Avoid => CellValue::Unknown,
            CellValue::Unknown => CellValue::Choose,
            CellValue::Choose => CellValue::Avoid,
        }
    }

    pub fn symbol(&self) -> char {
        match self {
            CellValue::Avoid => '░',
            CellValue::Unknown => '▒',
            CellValue::Choose => '█',
        }
    }
}

/// N×M grid of ternary cells
pub struct TernaryGrid {
    cells: Vec<Vec<CellValue>>,
    rows: usize,
    cols: usize,
}

impl TernaryGrid {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            cells: vec![vec![CellValue::Unknown; cols]; rows],
            rows,
            cols,
        }
    }

    pub fn get(&self, row: usize, col: usize) -> Option<CellValue> {
        self.cells.get(row).and_then(|r| r.get(col).copied())
    }

    pub fn set(&mut self, row: usize, col: usize, value: CellValue) {
        if let Some(r) = self.cells.get_mut(row) {
            if let Some(c) = r.get_mut(col) {
                *c = value;
            }
        }
    }

    pub fn cycle(&mut self, row: usize, col: usize) -> Option<CellValue> {
        if let Some(current) = self.get(row, col) {
            let next = current.next();
            self.set(row, col, next);
            Some(next)
        } else {
            None
        }
    }

    /// Compute conservation metrics for the grid
    pub fn conservation(&self) -> ConservationMetrics {
        let mut avoid = 0u32;
        let mut unknown = 0u32;
        let mut choose = 0u32;
        let total = (self.rows * self.cols) as u32;

        for row in &self.cells {
            for cell in row {
                match cell {
                    CellValue::Avoid => avoid += 1,
                    CellValue::Unknown => unknown += 1,
                    CellValue::Choose => choose += 1,
                }
            }
        }

        ConservationMetrics {
            avoid_ratio: avoid as f64 / total as f64,
            unknown_ratio: unknown as f64 / total as f64,
            choose_ratio: choose as f64 / total as f64,
            avoid_choose_ratio: if choose > 0 { avoid as f64 / choose as f64 } else { 0.0 },
        }
    }

    /// Render as ASCII (for TUI mode)
    pub fn render_ascii(&self) -> String {
        let mut s = String::new();
        for row in &self.cells {
            for cell in row {
                s.push(cell.symbol());
            }
            s.push('\n');
        }
        s
    }

    /// Export as JSON (for frontend)
    pub fn to_json(&self) -> String {
        let cells: Vec<Vec<i8>> = self.cells.iter()
            .map(|row| row.iter().map(|c| c.value()).collect())
            .collect();
        format!(r#"{{"rows":{},"cols":{},"cells":{:?}}}"#, self.rows, self.cols, cells)
    }
}

#[derive(Debug, Clone)]
pub struct ConservationMetrics {
    pub avoid_ratio: f64,
    pub unknown_ratio: f64,
    pub choose_ratio: f64,
    pub avoid_choose_ratio: f64,
}

impl ConservationMetrics {
    pub fn is_conserved(&self) -> bool {
        // Conservation law: ratios should be stable
        self.avoid_ratio > 0.0 && self.avoid_ratio < 1.0
    }

    pub fn summary(&self) -> String {
        format!(
            "Avoid: {:.1}% | Unknown: {:.1}% | Choose: {:.1}% | A/C ratio: {:.1}:1 | {}",
            self.avoid_ratio * 100.0,
            self.unknown_ratio * 100.0,
            self.choose_ratio * 100.0,
            self.avoid_choose_ratio,
            if self.is_conserved() { "✓ CONSERVED" } else { "✗ VIOLATED" }
        )
    }
}

/// The ternary engine exposed to the Tauri frontend
pub struct TernaryEngine {
    grid: TernaryGrid,
}

impl TernaryEngine {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            grid: TernaryGrid::new(rows, cols),
        }
    }

    /// Handle a cell click from the frontend
    pub fn click(&mut self, row: usize, col: usize) -> String {
        self.grid.cycle(row, col);
        self.grid.to_json()
    }

    /// Get current grid state as JSON
    pub fn state(&self) -> String {
        self.grid.to_json()
    }

    /// Get conservation metrics
    pub fn metrics(&self) -> ConservationMetrics {
        self.grid.conservation()
    }

    /// Reset grid
    pub fn reset(&mut self) {
        self.grid = TernaryGrid::new(self.grid.rows, self.grid.cols);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_cycle() {
        assert_eq!(CellValue::Avoid.next(), CellValue::Unknown);
        assert_eq!(CellValue::Unknown.next(), CellValue::Choose);
        assert_eq!(CellValue::Choose.next(), CellValue::Avoid);
    }

    #[test]
    fn test_grid_new() {
        let g = TernaryGrid::new(3, 4);
        assert_eq!(g.get(0, 0), Some(CellValue::Unknown));
        assert_eq!(g.get(2, 3), Some(CellValue::Unknown));
    }

    #[test]
    fn test_grid_set_get() {
        let mut g = TernaryGrid::new(3, 4);
        g.set(1, 2, CellValue::Choose);
        assert_eq!(g.get(1, 2), Some(CellValue::Choose));
    }

    #[test]
    fn test_grid_cycle() {
        let mut g = TernaryGrid::new(3, 4);
        let next = g.cycle(0, 0);
        assert_eq!(next, Some(CellValue::Choose)); // Unknown → Choose
    }

    #[test]
    fn test_conservation_initial() {
        let g = TernaryGrid::new(4, 5);
        let m = g.conservation();
        assert!((m.unknown_ratio - 1.0).abs() < 0.01);
        assert!((m.avoid_ratio - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_conservation_mixed() {
        let mut g = TernaryGrid::new(2, 4);
        g.set(0, 0, CellValue::Avoid);
        g.set(0, 1, CellValue::Avoid);
        g.set(0, 2, CellValue::Choose);
        g.set(0, 3, CellValue::Choose);
        g.set(1, 0, CellValue::Unknown);
        g.set(1, 1, CellValue::Unknown);
        g.set(1, 2, CellValue::Unknown);
        g.set(1, 3, CellValue::Unknown);
        let m = g.conservation();
        assert!((m.avoid_ratio - 0.25).abs() < 0.01);
        assert!((m.choose_ratio - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_render_ascii() {
        let mut g = TernaryGrid::new(1, 3);
        g.set(0, 0, CellValue::Avoid);
        g.set(0, 1, CellValue::Unknown);
        g.set(0, 2, CellValue::Choose);
        let rendered = g.render_ascii();
        assert!(rendered.contains("░▒█"));
    }

    #[test]
    fn test_to_json() {
        let g = TernaryGrid::new(1, 2);
        let json = g.to_json();
        assert!(json.contains("\"rows\":1"));
        assert!(json.contains("\"cols\":2"));
    }

    #[test]
    fn test_engine_click() {
        let mut e = TernaryEngine::new(3, 3);
        let result = e.click(1, 1);
        assert!(result.contains("1")); // Choose value
    }

    #[test]
    fn test_engine_reset() {
        let mut e = TernaryEngine::new(3, 3);
        e.click(0, 0);
        e.reset();
        let m = e.metrics();
        assert!((m.unknown_ratio - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_metrics_summary() {
        let mut g = TernaryGrid::new(2, 2);
        g.set(0, 0, CellValue::Choose);
        g.set(0, 1, CellValue::Choose);
        g.set(1, 0, CellValue::Avoid);
        g.set(1, 1, CellValue::Unknown);
        let m = g.conservation();
        let s = m.summary();
        assert!(s.contains("50.0%"));
    }
}
