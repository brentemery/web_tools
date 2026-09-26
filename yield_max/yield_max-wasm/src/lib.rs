use wasm_bindgen::prelude::*;

use yield_max_core::{
    col_label, find_best_region_with, mask_site_count, render_report, BestRegion, Grade, TieBreak,
    WaferMap, BOARD_SIZE, LEGEND, MASK_TEMPLATE, ROW_LABELS,
};

/// Scored placement of the 200mm region, carrying the full breakdown of why
/// it scored as it did rather than just the good-die count.
#[wasm_bindgen]
#[derive(Clone)]
pub struct Placement {
    row: usize,
    col: usize,
    /// Per-grade good counts, indexed by grade - 1. Exposed through flat
    /// getters rather than as a map, which is cheaper across the wasm boundary
    /// and matches the rest of this struct.
    good: [usize; yield_max_core::GRADES],
    defect: usize,
    overhang: usize,
    yield_fraction: f64,
    /// The region's center die, kept as a resolved position rather than
    /// recomputed in JS, so the UI cannot disagree with the report about
    /// where the center is.
    center_row: usize,
    center_col: usize,
    label: String,
    center_label: String,
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

    /// Good die of every grade. Keeps the meaning it had before grades
    /// existed, so a caller reading `good` is not silently handed a subset.
    #[wasm_bindgen(getter)]
    pub fn good(&self) -> usize {
        self.good.iter().sum()
    }

    /// Grade-4 good die -- the figure the solver maximizes.
    #[wasm_bindgen(getter)]
    pub fn good4(&self) -> usize {
        self.good[3]
    }

    #[wasm_bindgen(getter)]
    pub fn good3(&self) -> usize {
        self.good[2]
    }

    #[wasm_bindgen(getter)]
    pub fn good2(&self) -> usize {
        self.good[1]
    }

    #[wasm_bindgen(getter)]
    pub fn good1(&self) -> usize {
        self.good[0]
    }

    /// The region's top-left corner in the version-4 site notation (`"C5"`).
    #[wasm_bindgen(getter)]
    pub fn label(&self) -> String {
        self.label.clone()
    }

    /// Grid row of the region's center die.
    #[wasm_bindgen(getter)]
    pub fn center_row(&self) -> usize {
        self.center_row
    }

    /// Grid column of the region's center die.
    #[wasm_bindgen(getter)]
    pub fn center_col(&self) -> usize {
        self.center_col
    }

    /// The center die in the version-4 site notation (`"H10"`).
    #[wasm_bindgen(getter)]
    pub fn center_label(&self) -> String {
        self.center_label.clone()
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
        self.good() + self.defect + self.overhang
    }

    /// Good die as a fraction of present die, in 0.0..=1.0.
    #[wasm_bindgen(getter)]
    pub fn yield_fraction(&self) -> f64 {
        self.yield_fraction
    }
}

impl From<&BestRegion> for Placement {
    fn from(p: &BestRegion) -> Self {
        let (center_row, center_col) = p.center();
        Placement {
            row: p.row,
            col: p.col,
            good: p.stats.by_grade(),
            defect: p.stats.defect,
            overhang: p.stats.overhang,
            yield_fraction: p.stats.yield_fraction(),
            center_row,
            center_col,
            label: p.name(),
            center_label: p.center_name(),
        }
    }
}

#[wasm_bindgen]
pub struct AnalysisResult {
    best: Placement,
    report: String,
    warning: Option<String>,
    tie_break: TieBreak,
}

#[wasm_bindgen]
impl AnalysisResult {
    #[wasm_bindgen(getter)]
    pub fn best(&self) -> Placement {
        self.best.clone()
    }

    /// A non-fatal advisory, or the empty string. Currently set when the
    /// input carried region marks that this run will overwrite.
    #[wasm_bindgen(getter)]
    pub fn warning(&self) -> String {
        self.warning.clone().unwrap_or_default()
    }

    /// The full self-describing report: `#` header plus the marked grid.
    #[wasm_bindgen(getter)]
    pub fn report(&self) -> String {
        self.report.clone()
    }

    /// The tie-break policy that produced this result, so the UI can label the
    /// region with the policy behind it rather than assuming the default.
    #[wasm_bindgen(getter)]
    pub fn tiebreak(&self) -> String {
        self.tie_break.as_str().to_string()
    }
}

/// Finds the 200mm region covering the most grade-4 die.
///
/// `tie_break` names the policy for settling a tie on the grade-4 count
/// (`"grade"` or `"total"`); it is optional and trailing so the original
/// one-argument call still works, and `null`/`undefined`/`""` mean "use the
/// default". An unrecognized value throws rather than falling back, since a
/// silent fallback would answer a different question than the one asked.
#[wasm_bindgen]
pub fn analyze_wafer(input: &str, tie_break: Option<String>) -> Result<AnalysisResult, JsValue> {
    let tie_break = match tie_break.as_deref() {
        None | Some("") => TieBreak::default(),
        Some(name) => name
            .parse::<TieBreak>()
            .map_err(|e| JsValue::from_str(&e.to_string()))?,
    };

    let map = WaferMap::parse(input).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let warning = map.has_inconsistent_marks().then(|| {
        "This map contains region marks that match no legal 200mm placement; \
         they have been replaced by this run's result."
            .to_string()
    });
    let best = find_best_region_with(&map, tie_break).ok_or_else(|| {
        JsValue::from_str(
            "no 200mm region fits entirely within this wafer with at least one die of \
             clearance from the wafer's edge on every side \
             (its present-die area is too small everywhere it could sit)",
        )
    })?;

    Ok(AnalysisResult {
        best: Placement::from(&best),
        report: render_report(&map, &best, tie_break),
        warning,
        tie_break,
    })
}

/// The legal `tie_break` values, so the UI builds its control from the solver's
/// own list instead of hard-coding one that could drift.
#[wasm_bindgen]
pub fn tie_breaks() -> Vec<String> {
    TieBreak::ALL
        .iter()
        .map(|t| t.as_str().to_string())
        .collect()
}

/// The number of good-die grades, highest first (`[4, 3, 2, 1]`), so the UI can
/// enumerate grades without assuming how many there are.
#[wasm_bindgen]
pub fn grades_best_first() -> Vec<u8> {
    Grade::BEST_FIRST.iter().map(|g| g.number()).collect()
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

/// The row letters, top to bottom, with `N` skipped. Exported so the web UI
/// labels its axis from the solver's own list instead of re-deriving the
/// skip rule and drifting.
#[wasm_bindgen]
pub fn row_labels() -> Vec<String> {
    ROW_LABELS.iter().map(|c| c.to_string()).collect()
}

/// The column numbers, left to right. Trivial today, but exported beside
/// `row_labels()` so both axes come from one place.
#[wasm_bindgen]
pub fn col_labels() -> Vec<usize> {
    (0..BOARD_SIZE).map(col_label).collect()
}
