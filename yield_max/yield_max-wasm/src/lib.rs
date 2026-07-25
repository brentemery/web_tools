use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct AnalysisResult {
    row: usize,
    col: usize,
    good_die_count: usize,
    marked_map: String,
}

#[wasm_bindgen]
impl AnalysisResult {
    #[wasm_bindgen(getter)]
    pub fn row(&self) -> usize {
        self.row
    }

    #[wasm_bindgen(getter)]
    pub fn col(&self) -> usize {
        self.col
    }

    #[wasm_bindgen(getter)]
    pub fn good_die_count(&self) -> usize {
        self.good_die_count
    }

    #[wasm_bindgen(getter)]
    pub fn marked_map(&self) -> String {
        self.marked_map.clone()
    }
}

#[wasm_bindgen]
pub fn analyze_wafer(input: &str) -> Result<AnalysisResult, JsValue> {
    let map = yield_max_core::WaferMap::parse(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let best = yield_max_core::find_best_region(&map);
    let marked_map = yield_max_core::mark_region(&map, &best);

    Ok(AnalysisResult {
        row: best.row,
        col: best.col,
        good_die_count: best.good_die_count,
        marked_map,
    })
}
