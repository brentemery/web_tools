use wasm_bindgen::prelude::*;

use yield_max_core::{
    find_best_region, mask_site_count, render_report, BestRegion, WaferMap, LEGEND, MASK_TEMPLATE,
};

/// Scored placement of the 200mm region, carrying the full breakdown of why
/// it scored as it did rather than just the good-die count.
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct Placement {
    row: usize,
    col: usize,
    good: usize,
    defect: usize,
    overhang: usize,
    yield_fraction: f64,
}

#[wasm_bindgen]
impl Placement {
    #[wasm_bindgen(getter)]
    pub fn row(&self) -> usize {
        self.row
    }

    #[wasm_bindgen(getter)]
    pub fn col(&self) -> usize {
        self.col
    }

    #[wasm_bindgen(getter)]
    pub fn good(&self) -> usize {
        self.good
    }

    #[wasm_bindgen(getter)]
    pub fn defect(&self) -> usize {
        self.defect
    }

    #[wasm_bindgen(getter)]
    pub fn overhang(&self) -> usize {
        self.overhang
    }

    #[wasm_bindgen(getter)]
    pub fn sites(&self) -> usize {
        self.good + self.defect + self.overhang
    }

    /// Good die as a fraction of present die, in 0.0..=1.0.
    #[wasm_bindgen(getter)]
    pub fn yield_fraction(&self) -> f64 {
        self.yield_fraction
    }
}

impl From<&BestRegion> for Placement {
    fn from(p: &BestRegion) -> Self {
        Placement {
            row: p.row,
            col: p.col,
            good: p.stats.good,
            defect: p.stats.defect,
            overhang: p.stats.overhang,
            yield_fraction: p.stats.yield_fraction(),
        }
    }
}

#[wasm_bindgen]
pub struct AnalysisResult {
    best: Placement,
    report: String,
}

#[wasm_bindgen]
impl AnalysisResult {
    #[wasm_bindgen(getter)]
    pub fn best(&self) -> Placement {
        self.best
    }

    /// The full self-describing report: `#` header plus the marked grid.
    #[wasm_bindgen(getter)]
    pub fn report(&self) -> String {
        self.report.clone()
    }
}

#[wasm_bindgen]
pub fn analyze_wafer(input: &str) -> Result<AnalysisResult, JsValue> {
    let map = WaferMap::parse(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let best = find_best_region(&map);

    Ok(AnalysisResult {
        best: Placement::from(&best),
        report: render_report(&map, &best),
    })
}

/// The 200mm mask footprint as `O`/`.` rows. Exported so the web UI can draw
/// the region outline from the same constant the solver uses, instead of
/// keeping a copy that could silently drift out of sync.
#[wasm_bindgen]
pub fn mask_rows() -> Vec<String> {
    MASK_TEMPLATE.iter().map(|r| r.to_string()).collect()
}

/// Total die sites a 200mm region occupies, wherever it is placed.
#[wasm_bindgen]
pub fn mask_sites() -> usize {
    mask_site_count()
}

/// The cell-alphabet legend, so the UI never has to restate it.
#[wasm_bindgen]
pub fn legend() -> String {
    LEGEND.to_string()
}
