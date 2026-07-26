//! Core solver for the Yield Max wafer-yield problem: given a 300mm wafer
//! map, find the highest-yielding placement of the fixed 200mm mask.
//!
//! # File format (version 2)
//!
//! A wafer map is a 17x17 grid of single characters. Each cell encodes two
//! orthogonal facts: the state of the die, and whether the cell falls inside
//! a marked 200mm region.
//!
//! |         | outside region | inside region |
//! |---------|----------------|---------------|
//! | good    | `1`            | `Z`           |
//! | defect  | `X`            | `*`           |
//! | absent  | `.`            | `-`           |
//!
//! Lines beginning with `#` are comments and are ignored on input; the
//! renderer emits a three-line `#` header describing the result so that the
//! output file is self-describing. Because the alphabet is lossless, output
//! of this tool is valid input to it (see [`WaferMap::marked_region`]).

pub const BOARD_SIZE: usize = 17;
pub const MASK_SIZE: usize = 11;

/// The fixed 200mm region footprint, as `O`/`.` rows. This is the single
/// source of truth for the mask shape; the web UI reads it back out through
/// the WASM `mask_rows()` export rather than keeping its own copy.
pub const MASK_TEMPLATE: [&str; MASK_SIZE] = [
    "...OOOOO...",
    "..OOOOOOO..",
    ".OOOOOOOOO.",
    "OOOOOOOOOOO",
    "OOOOOOOOOOO",
    "OOOOOOOOOOO",
    "OOOOOOOOOOO",
    ".OOOOOOOOO.",
    ".OOOOOOOOO.",
    "..OOOOOOO..",
    "....OOO....",
];

/// Human-readable legend for the version-2 cell alphabet, emitted in the
/// output header so a reader needs no external documentation.
pub const LEGEND: &str =
    "in-region: Z=good *=defect -=overhang   outside: 1=good X=defect .=absent";

fn mask() -> [[bool; MASK_SIZE]; MASK_SIZE] {
    let mut grid = [[false; MASK_SIZE]; MASK_SIZE];
    for (r, row) in MASK_TEMPLATE.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            grid[r][c] = ch == 'O';
        }
    }
    grid
}

/// Number of `O` cells in the mask, i.e. the number of die sites a 200mm
/// region occupies regardless of where it is placed.
pub fn mask_site_count() -> usize {
    MASK_TEMPLATE
        .iter()
        .map(|row| row.chars().filter(|&c| c == 'O').count())
        .sum()
}

/// The state of a single die site, independent of region membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Die {
    Good,
    Defect,
    Absent,
}

impl Die {
    fn from_char(ch: char) -> Option<(Die, bool)> {
        match ch {
            '1' => Some((Die::Good, false)),
            'X' => Some((Die::Defect, false)),
            '.' => Some((Die::Absent, false)),
            'Z' => Some((Die::Good, true)),
            '*' => Some((Die::Defect, true)),
            '-' => Some((Die::Absent, true)),
            _ => None,
        }
    }

    /// The glyph for this die given whether it lies inside a marked region.
    pub fn to_char(self, in_region: bool) -> char {
        match (self, in_region) {
            (Die::Good, false) => '1',
            (Die::Defect, false) => 'X',
            (Die::Absent, false) => '.',
            (Die::Good, true) => 'Z',
            (Die::Defect, true) => '*',
            (Die::Absent, true) => '-',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaferMap {
    grid: Vec<Vec<Die>>,
    /// Cells that arrived already marked as in-region, used by
    /// [`WaferMap::marked_region`] to recover a previous run's placement.
    marked: Vec<Vec<bool>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    WrongRowCount(usize),
    WrongRowLength { row: usize, len: usize },
    InvalidChar { row: usize, col: usize, ch: char },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Rows and columns are reported 1-based to match what a text editor
        // shows the user.
        match self {
            ParseError::WrongRowCount(n) => {
                write!(f, "expected {BOARD_SIZE} wafer rows, found {n}")
            }
            ParseError::WrongRowLength { row, len } => {
                write!(
                    f,
                    "row {} has length {len}, expected {BOARD_SIZE}",
                    row + 1
                )
            }
            ParseError::InvalidChar { row, col, ch } => write!(
                f,
                "row {}, col {}: invalid character '{ch}' (expected one of '.', 'X', '1', 'Z', '*', '-')",
                row + 1,
                col + 1
            ),
        }
    }
}

impl std::error::Error for ParseError {}

impl WaferMap {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        // Drop `#` comment lines anywhere, but only trim blank lines at the
        // ends: a blank line in the middle of the grid is a malformed file,
        // not something to silently paper over by shifting rows up.
        let mut rows: Vec<&str> = input
            .lines()
            .map(|l| l.trim_end_matches('\r'))
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect();
        while rows.first().is_some_and(|l| l.trim().is_empty()) {
            rows.remove(0);
        }
        while rows.last().is_some_and(|l| l.trim().is_empty()) {
            rows.pop();
        }

        if rows.len() != BOARD_SIZE {
            return Err(ParseError::WrongRowCount(rows.len()));
        }

        let mut grid = Vec::with_capacity(BOARD_SIZE);
        let mut marked = Vec::with_capacity(BOARD_SIZE);
        for (r, row) in rows.iter().enumerate() {
            // Trailing whitespace is a plausible accident in hand-edited
            // ASCII art, so tolerate it rather than failing on row length.
            let chars: Vec<char> = row.trim_end().chars().collect();
            if chars.len() != BOARD_SIZE {
                return Err(ParseError::WrongRowLength {
                    row: r,
                    len: chars.len(),
                });
            }

            let mut die_row = Vec::with_capacity(BOARD_SIZE);
            let mut mark_row = Vec::with_capacity(BOARD_SIZE);
            for (c, &ch) in chars.iter().enumerate() {
                let (die, in_region) = Die::from_char(ch)
                    .ok_or(ParseError::InvalidChar { row: r, col: c, ch })?;
                die_row.push(die);
                mark_row.push(in_region);
            }
            grid.push(die_row);
            marked.push(mark_row);
        }

        Ok(WaferMap { grid, marked })
    }

    pub fn get(&self, row: usize, col: usize) -> Die {
        self.grid[row][col]
    }

    /// Recovers the region recorded in a previously marked file, if the
    /// marked cells exactly match some legal placement of the mask. Returns
    /// `None` for an unmarked map, or if the marks do not form a valid
    /// footprint (a hand-edited file, say).
    pub fn marked_region(&self) -> Option<BestRegion> {
        if self.marked.iter().flatten().all(|&m| !m) {
            return None;
        }
        let mask = mask();
        for row in 0..=(BOARD_SIZE - MASK_SIZE) {
            for col in 0..=(BOARD_SIZE - MASK_SIZE) {
                let matches = (0..BOARD_SIZE).all(|r| {
                    (0..BOARD_SIZE).all(|c| {
                        let inside = r >= row
                            && r < row + MASK_SIZE
                            && c >= col
                            && c < col + MASK_SIZE
                            && mask[r - row][c - col];
                        inside == self.marked[r][c]
                    })
                });
                if matches {
                    return Some(self.evaluate(row, col));
                }
            }
        }
        None
    }

    /// Scores a single placement of the mask with its top-left corner at
    /// (`row`, `col`).
    fn evaluate(&self, row: usize, col: usize) -> BestRegion {
        let mask = mask();
        let mut stats = RegionStats::default();
        for (dr, mask_row) in mask.iter().enumerate() {
            for (dc, &covered) in mask_row.iter().enumerate() {
                if !covered {
                    continue;
                }
                match self.grid[row + dr][col + dc] {
                    Die::Good => stats.good += 1,
                    Die::Defect => stats.defect += 1,
                    Die::Absent => stats.overhang += 1,
                }
            }
        }
        BestRegion { row, col, stats }
    }
}

/// Why a placement scored the way it did: how many good die it captures, how
/// many defect die it is forced to carry, and how many of its sites hang off
/// the edge of the wafer onto nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RegionStats {
    pub good: usize,
    pub defect: usize,
    pub overhang: usize,
}

impl RegionStats {
    /// Total die sites the region occupies. Always equals [`mask_site_count`].
    pub fn sites(&self) -> usize {
        self.good + self.defect + self.overhang
    }

    /// Good die as a fraction of *present* die (good + defect), i.e. the
    /// yield of the silicon actually under the region. Overhang is excluded
    /// because an absent site is not a failed die. Returns 0.0 if the region
    /// covers no present die at all.
    pub fn yield_fraction(&self) -> f64 {
        let present = self.good + self.defect;
        if present == 0 {
            0.0
        } else {
            self.good as f64 / present as f64
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BestRegion {
    pub row: usize,
    pub col: usize,
    pub stats: RegionStats,
}

/// Returns the placement covering the most good ('1') die, scanning every
/// position where the 11x11 mask fits fully inside the 17x17 board. Ties are
/// broken in favor of the first placement found (row-major order).
pub fn find_best_region(map: &WaferMap) -> BestRegion {
    let max_offset = BOARD_SIZE - MASK_SIZE;
    // The board is strictly larger than the mask, so (0, 0) is always a legal
    // placement and this seed is always replaced-or-matched by a real score.
    let mut best = map.evaluate(0, 0);
    for row in 0..=max_offset {
        for col in 0..=max_offset {
            let candidate = map.evaluate(row, col);
            // Strict `>` keeps the first placement in row-major order on ties.
            if candidate.stats.good > best.stats.good {
                best = candidate;
            }
        }
    }
    best
}

/// Renders `map` with the winning region's cells rewritten in the in-region
/// alphabet (`Z`/`*`/`-`), without the `#` header. Absent cells under the
/// mask become `-` to make wasted overhang sites visible.
pub fn mark_region(map: &WaferMap, region: &BestRegion) -> String {
    let mask = mask();
    let mut out = String::with_capacity(BOARD_SIZE * (BOARD_SIZE + 1));

    for r in 0..BOARD_SIZE {
        for c in 0..BOARD_SIZE {
            let in_region = r >= region.row
                && r < region.row + MASK_SIZE
                && c >= region.col
                && c < region.col + MASK_SIZE
                && mask[r - region.row][c - region.col];
            out.push(map.get(r, c).to_char(in_region));
        }
        out.push('\n');
    }

    out
}

/// Renders the full self-describing report: a three-line `#` header carrying
/// the headline numbers and the legend, followed by the marked grid.
pub fn render_report(map: &WaferMap, region: &BestRegion) -> String {
    let s = &region.stats;
    format!(
        "# yield_max 2  region=row{},col{}\n\
         # good={} defect={} overhang={} sites={} yield={:.1}%\n\
         # {}\n{}",
        region.row,
        region.col,
        s.good,
        s.defect,
        s.overhang,
        s.sites(),
        s.yield_fraction() * 100.0,
        LEGEND,
        mark_region(map, region),
    )
}

#[cfg(test)]
mod tests;
