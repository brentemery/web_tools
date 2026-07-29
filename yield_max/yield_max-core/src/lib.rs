//! Core solver for the Yield Max wafer-yield problem: given a 300mm wafer
//! map, find the highest-yielding placement of the fixed 200mm mask.
//!
//! # File format (version 3)
//!
//! A wafer map is a 17x17 grid of single characters. Each cell encodes two
//! orthogonal facts: the state of the die, and whether the cell falls inside
//! a marked 200mm region.
//!
//! A good die carries a **grade** in 1..=4 (a bin or speed grade). `1` is a
//! grade-1 good die, not a deprecated "ungraded" spelling, so every version-2
//! wafer map is still valid input with an unchanged answer.
//!
//! |         | outside region | inside region |
//! |---------|----------------|---------------|
//! | good 1  | `1`            | `A`           |
//! | good 2  | `2`            | `B`           |
//! | good 3  | `3`            | `C`           |
//! | good 4  | `4`            | `D`           |
//! | defect  | `X`            | `*`           |
//! | absent  | `.`            | `-`           |
//!
//! Grade order maps to alphabet order so the in-region glyph is readable
//! without the legend. Version 2's `Z` (its only in-region good glyph) is
//! still accepted on input as an in-region grade-1 die, but is never emitted.
//!
//! The objective is to maximize the number of **grade-4** die under the
//! region; how a tie on that count is settled is the caller's choice (see
//! [`TieBreak`]).
//!
//! Lines beginning with `#` are comments and are ignored on input; the
//! renderer emits a three-line `#` header describing the result so that the
//! output file is self-describing. Because the alphabet is lossless, output
//! of this tool is valid input to it (see [`WaferMap::marked_region`]).
//!
//! Free-text header lines above the grid (lot number, operator, timestamp --
//! anything not marked with `#`) are also tolerated: any leading line that
//! isn't 17 characters wide is dropped before the grid is parsed.

mod html;
pub use html::render_html;

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

/// Human-readable legend for the version-3 cell alphabet, emitted in the
/// output header so a reader needs no external documentation.
pub const LEGEND: &str = "in-region: D=good4 C=good3 B=good2 A=good1 *=defect -=overhang   \
                          outside: 4/3/2/1=good X=defect .=absent";

/// Number of distinct good-die grades, 1..=[`GRADES`].
pub const GRADES: usize = 4;

/// The grade (bin) of a good die, 1..=4. Higher is better; the solver's
/// objective is to maximize the count of grade 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Grade {
    G1 = 1,
    G2 = 2,
    G3 = 3,
    G4 = 4,
}

impl Grade {
    /// All grades, best first -- the order the lexicographic comparison and
    /// the report header both walk, so neither hard-codes the sequence.
    pub const BEST_FIRST: [Grade; GRADES] = [Grade::G4, Grade::G3, Grade::G2, Grade::G1];

    /// The grade as a number in 1..=4.
    pub fn number(self) -> u8 {
        self as u8
    }

    /// Builds a grade from a number in 1..=4, or `None`.
    pub fn from_number(n: u8) -> Option<Grade> {
        match n {
            1 => Some(Grade::G1),
            2 => Some(Grade::G2),
            3 => Some(Grade::G3),
            4 => Some(Grade::G4),
            _ => None,
        }
    }

    /// Index into a per-grade array, 0-based.
    fn index(self) -> usize {
        self as usize - 1
    }
}

/// How to settle a tie on the grade-4 count. Maximizing grade 4 is the fixed
/// objective; this only decides what to prefer among placements that capture
/// equally many grade-4 die, and the two answers serve different users -- top
/// bin volume versus total sellable die. Both collapse to "most good die" on a
/// wafer that uses only grade 1, so neither changes a version-2 answer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TieBreak {
    /// Lexicographic by grade: (n4, n3, n2, n1) descending. The strict reading
    /// of "prefer better silicon", and the default.
    #[default]
    Grade,
    /// (n4, total good) descending, then (n3, n2, n1) so the order stays total
    /// and the result deterministic.
    Total,
}

impl TieBreak {
    /// The spelling used on the command line, in the report header, and in
    /// JSON -- one function so those three can never disagree.
    pub fn as_str(self) -> &'static str {
        match self {
            TieBreak::Grade => "grade",
            TieBreak::Total => "total",
        }
    }

    /// Every legal spelling, for error messages and UI.
    pub const ALL: [TieBreak; 2] = [TieBreak::Grade, TieBreak::Total];
}

impl std::fmt::Display for TieBreak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Rejected tie-break spelling. Carries the offending value so callers can
/// echo it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownTieBreak(pub String);

impl std::fmt::Display for UnknownTieBreak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = TieBreak::ALL.iter().map(|t| t.as_str()).collect();
        write!(
            f,
            "unknown tie-break {:?}; expected one of {}",
            self.0,
            names.join(", ")
        )
    }
}

impl std::error::Error for UnknownTieBreak {}

impl std::str::FromStr for TieBreak {
    type Err = UnknownTieBreak;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TieBreak::ALL
            .into_iter()
            .find(|t| t.as_str() == s)
            .ok_or_else(|| UnknownTieBreak(s.to_string()))
    }
}

/// The mask as a boolean grid, derived once from [`MASK_TEMPLATE`]. Deriving
/// it from the string keeps a single human-readable source of truth for the
/// shape; caching it stops the solver re-parsing that string on each of the 49
/// placements it evaluates, which is roughly half the total solve time.
static MASK: std::sync::LazyLock<[[bool; MASK_SIZE]; MASK_SIZE]> = std::sync::LazyLock::new(|| {
    let mut grid = [[false; MASK_SIZE]; MASK_SIZE];
    for (r, row) in MASK_TEMPLATE.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            grid[r][c] = ch == 'O';
        }
    }
    grid
});

fn mask() -> &'static [[bool; MASK_SIZE]; MASK_SIZE] {
    &MASK
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
    Good(Grade),
    Defect,
    Absent,
}

/// Version 2's only in-region good glyph. Still accepted on input so a v2
/// report round-trips, but never emitted: v3 spells an in-region grade-1 die
/// `A`.
pub const LEGACY_IN_REGION_GOOD: char = 'Z';

impl Die {
    fn from_char(ch: char) -> Option<(Die, bool)> {
        match ch {
            '1'..='4' => {
                let n = ch as u8 - b'0';
                Some((Die::Good(Grade::from_number(n)?), false))
            }
            'A'..='D' => {
                let n = ch as u8 - b'A' + 1;
                Some((Die::Good(Grade::from_number(n)?), true))
            }
            LEGACY_IN_REGION_GOOD => Some((Die::Good(Grade::G1), true)),
            'X' => Some((Die::Defect, false)),
            '.' => Some((Die::Absent, false)),
            '*' => Some((Die::Defect, true)),
            '-' => Some((Die::Absent, true)),
            _ => None,
        }
    }

    /// The glyph for this die given whether it lies inside a marked region.
    pub fn to_char(self, in_region: bool) -> char {
        match (self, in_region) {
            // Grade order maps to digit and letter order, so both spellings
            // are derived rather than tabulated.
            (Die::Good(g), false) => (b'0' + g.number()) as char,
            (Die::Good(g), true) => (b'A' + g.number() - 1) as char,
            (Die::Defect, false) => 'X',
            (Die::Absent, false) => '.',
            (Die::Defect, true) => '*',
            (Die::Absent, true) => '-',
        }
    }

    /// The grade, if this is a good die.
    pub fn grade(self) -> Option<Grade> {
        match self {
            Die::Good(g) => Some(g),
            _ => None,
        }
    }

    /// True for any good die, whatever its grade.
    pub fn is_good(self) -> bool {
        matches!(self, Die::Good(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaferMap {
    grid: Vec<Vec<Die>>,
    /// Cells that arrived already marked as in-region, used by
    /// [`WaferMap::marked_region`] to recover a previous run's placement.
    marked: Vec<Vec<bool>>,
    /// The `tiebreak=` value found in a `#` header line, if the input was a
    /// report from an earlier run. `None` if absent; `Some(Err(..))` if
    /// present but unrecognized, which is a caller-visible problem rather
    /// than something to guess about.
    header_tie_break: Option<Result<TieBreak, UnknownTieBreak>>,
}

/// Largest input we will look at. A valid map is ~300 bytes; this leaves room
/// for a generous comment header while refusing to allocate for a file that
/// cannot possibly be a wafer map.
pub const MAX_INPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    TooLarge { bytes: usize },
    WrongRowCount(usize),
    WrongRowLength { row: usize, len: usize },
    InvalidChar { row: usize, col: usize, ch: char },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Rows and columns are reported 1-based to match what a text editor
        // shows the user.
        match self {
            ParseError::TooLarge { bytes } => write!(
                f,
                "input is {bytes} bytes, larger than the {MAX_INPUT_BYTES} byte limit; \
                 a wafer map is {BOARD_SIZE} rows of {BOARD_SIZE} characters"
            ),
            ParseError::WrongRowCount(n) => {
                write!(f, "expected {BOARD_SIZE} wafer rows, found {n}")
            }
            ParseError::WrongRowLength { row, len } => {
                write!(f, "row {} has length {len}, expected {BOARD_SIZE}", row + 1)
            }
            ParseError::InvalidChar { row, col, ch } => write!(
                f,
                "row {}, col {}: invalid character {} (expected one of \
                 '.', 'X', '1'..'4', 'A'..'D', 'Z', '*', '-')",
                row + 1,
                col + 1,
                describe_char(*ch)
            ),
        }
    }
}

impl std::error::Error for ParseError {}

/// Whether `line` has the width of a grid row (trailing whitespace ignored,
/// matching the tolerance the row parser itself applies). Used only to tell
/// leading header text apart from the grid, not to validate its contents.
fn is_grid_row_width(line: &str) -> bool {
    line.trim_end().chars().count() == BOARD_SIZE
}

/// Describes a character for an error message. Whitespace and non-printing
/// characters are named rather than printed, so a message about a tab, a
/// non-breaking space, or a zero-width character is not itself invisible.
fn describe_char(ch: char) -> String {
    let name = match ch {
        '\t' => Some("tab"),
        '\u{a0}' => Some("non-breaking space"),
        '\u{200b}' => Some("zero-width space"),
        '\u{feff}' => Some("byte-order mark"),
        ' ' => Some("space"),
        _ => None,
    };
    match name {
        Some(n) => format!("U+{:04X} ({n})", ch as u32),
        None if ch.is_control() || !ch.is_ascii() => {
            format!("'{}' (U+{:04X})", ch.escape_debug(), ch as u32)
        }
        None => format!("'{ch}'"),
    }
}

/// Finds `tiebreak=<value>` in a `#` comment line, as written by
/// [`render_report`]. Only comment lines are searched, so a grid row can never
/// be mistaken for a header.
fn parse_header_tie_break(rows: &[&str]) -> Option<Result<TieBreak, UnknownTieBreak>> {
    rows.iter()
        .filter(|l| l.trim_start().starts_with('#'))
        .flat_map(|l| l.split_whitespace())
        .find_map(|tok| tok.strip_prefix("tiebreak="))
        .map(|v| v.parse::<TieBreak>())
}

impl WaferMap {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        // Bail before allocating for something that cannot be a wafer map.
        if input.len() > MAX_INPUT_BYTES {
            return Err(ParseError::TooLarge { bytes: input.len() });
        }

        // A UTF-8 BOM is invisible in most editors, so left in place it would
        // produce a "row 1 has length 18" complaint about a row the user can
        // plainly see is 17 characters. Strip it instead.
        let input = input.strip_prefix('\u{feff}').unwrap_or(input);

        // Drop `#` comment lines, but only at the top and bottom: a comment
        // interleaved with the grid is as suspect as a blank line there, since
        // it suggests the file was assembled wrongly.
        let mut rows: Vec<&str> = input.lines().map(|l| l.trim_end_matches('\r')).collect();
        // Recover the tie-break policy from a previous run's header before
        // the comment lines are discarded.
        let header_tie_break = parse_header_tie_break(&rows);
        // Some source systems prepend free-text metadata (lot number,
        // operator, timestamp) above the grid with no `#` marker. Treat any
        // leading line that isn't 17 characters wide as header text and drop
        // it; a line that does happen to be 17 characters wide is left for
        // the normal per-character validation below, so a genuinely malformed
        // first grid row (wrong character, invisible character, ...) still
        // fails loudly instead of being swallowed as "header".
        while rows.first().is_some_and(|l| {
            l.trim().is_empty() || l.trim_start().starts_with('#') || !is_grid_row_width(l)
        }) {
            rows.remove(0);
        }
        while rows
            .last()
            .is_some_and(|l| l.trim().is_empty() || l.trim_start().starts_with('#'))
        {
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
                let (die, in_region) =
                    Die::from_char(ch).ok_or(ParseError::InvalidChar { row: r, col: c, ch })?;
                die_row.push(die);
                mark_row.push(in_region);
            }
            grid.push(die_row);
            marked.push(mark_row);
        }

        Ok(WaferMap {
            grid,
            marked,
            header_tie_break,
        })
    }

    /// The tie-break policy recorded in the input's `#` header, if any. A
    /// caller re-running on a previous report honors this when no explicit
    /// policy was requested, which is what keeps "our output is our input,
    /// byte for byte" true under either policy. `Some(Err(..))` means the
    /// header named something unrecognized.
    pub fn header_tie_break(&self) -> Option<&Result<TieBreak, UnknownTieBreak>> {
        self.header_tie_break.as_ref()
    }

    pub fn get(&self, row: usize, col: usize) -> Die {
        self.grid[row][col]
    }

    /// True if the input carried any in-region glyph (`Z`, `*`, `-`).
    pub fn has_marks(&self) -> bool {
        self.marked.iter().flatten().any(|&m| m)
    }

    /// True if the input carried marks that match no legal mask placement.
    /// Such marks are silently overwritten when the report is rendered, so
    /// callers should warn rather than destroy the user's edit unannounced.
    pub fn has_inconsistent_marks(&self) -> bool {
        self.has_marks() && self.marked_region().is_none()
    }

    /// Recovers the region recorded in a previously marked file, if the
    /// marked cells exactly match some legal placement of the mask. Returns
    /// `None` for an unmarked map, or if the marks do not form a valid
    /// footprint (a hand-edited file, say).
    pub fn marked_region(&self) -> Option<BestRegion> {
        if !self.has_marks() {
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
                    Die::Good(g) => stats.good[g.index()] += 1,
                    Die::Defect => stats.defect += 1,
                    Die::Absent => stats.overhang += 1,
                }
            }
        }
        BestRegion { row, col, stats }
    }

    /// True if the die at (`r`, `c`) sits on the edge of the wafer: off the
    /// 17x17 grid entirely, or adjacent -- including diagonally -- to an
    /// absent (`.`) cell. A legal 200mm region may not cover an edge die, so
    /// it always keeps at least one die of clearance from the wafer's true
    /// boundary.
    fn is_edge_die(&self, r: usize, c: usize) -> bool {
        for dr in -1i32..=1 {
            for dc in -1i32..=1 {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let (nr, nc) = (r as i32 + dr, c as i32 + dc);
                if nr < 0 || nc < 0 || nr as usize >= BOARD_SIZE || nc as usize >= BOARD_SIZE {
                    return true;
                }
                if self.grid[nr as usize][nc as usize] == Die::Absent {
                    return true;
                }
            }
        }
        false
    }

    /// True if any site the mask would cover at (`row`, `col`) is a wafer
    /// edge die (see [`WaferMap::is_edge_die`]), making this placement
    /// illegal regardless of how many good die it covers.
    fn region_touches_wafer_edge(&self, row: usize, col: usize) -> bool {
        let mask = mask();
        mask.iter().enumerate().any(|(dr, mask_row)| {
            mask_row
                .iter()
                .enumerate()
                .any(|(dc, &covered)| covered && self.is_edge_die(row + dr, col + dc))
        })
    }
}

/// Why a placement scored the way it did: how many good die of each grade it
/// captures, how many defect die it is forced to carry, and how many of its
/// sites hang off the edge of the wafer onto nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RegionStats {
    /// Good die counts indexed by grade - 1, so `good[3]` is the grade-4
    /// count. Private with accessors rather than a public field: the field
    /// used to be a bare `usize` total, and making every call site go through
    /// a named accessor is what lets the compiler find them all.
    good: [usize; GRADES],
    pub defect: usize,
    pub overhang: usize,
}

impl RegionStats {
    pub fn new(good: [usize; GRADES], defect: usize, overhang: usize) -> Self {
        RegionStats {
            good,
            defect,
            overhang,
        }
    }

    /// Good die of a single grade.
    pub fn grade(&self, grade: Grade) -> usize {
        self.good[grade.index()]
    }

    /// Per-grade counts indexed by grade - 1.
    pub fn by_grade(&self) -> [usize; GRADES] {
        self.good
    }

    /// Good die of every grade. This is what version 2 called `good`, and it
    /// keeps that meaning in the report header and in JSON.
    pub fn good_total(&self) -> usize {
        self.good.iter().sum()
    }

    /// Total die sites the region occupies. Always equals [`mask_site_count`].
    pub fn sites(&self) -> usize {
        self.good_total() + self.defect + self.overhang
    }

    /// Good die as a fraction of *present* die (good + defect), i.e. the
    /// yield of the silicon actually under the region. Overhang is excluded
    /// because an absent site is not a failed die. Returns 0.0 if the region
    /// covers no present die at all.
    pub fn yield_fraction(&self) -> f64 {
        let present = self.good_total() + self.defect;
        if present == 0 {
            0.0
        } else {
            self.good_total() as f64 / present as f64
        }
    }

    /// The comparison key: a fixed-width tuple, greater is better, so the
    /// solver has exactly one comparison site whichever policy is in force.
    /// The grade-4 count leads under both policies -- that part is the
    /// requirement, not a preference.
    pub fn sort_key(&self, tie_break: TieBreak) -> [usize; GRADES + 1] {
        let [n1, n2, n3, n4] = self.good;
        match tie_break {
            TieBreak::Grade => [n4, n3, n2, n1, 0],
            // Total leads after n4; the remaining grades follow so that two
            // placements with the same n4 and same total still order
            // deterministically instead of falling to the positional
            // tie-break by accident.
            TieBreak::Total => [n4, self.good_total(), n3, n2, n1],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BestRegion {
    pub row: usize,
    pub col: usize,
    pub stats: RegionStats,
}

/// Returns the placement covering the most **grade-4** good die, using
/// [`TieBreak::default`] to settle ties on that count. See
/// [`find_best_region_with`] for the full contract.
pub fn find_best_region(map: &WaferMap) -> Option<BestRegion> {
    find_best_region_with(map, TieBreak::default())
}

/// Returns the placement covering the most grade-4 good die, scanning every
/// position where the 11x11 mask fits fully inside the 17x17 board, entirely
/// on present die, and with at least one die of clearance from the wafer's
/// true edge on every side. A placement that would hang any mask site off the
/// wafer's physical edge onto an absent ('.') site is overhang and illegal;
/// a placement that would cover a die that itself sits on the wafer's edge
/// (off-grid or diagonally adjacent to an absent site) is also illegal, even
/// though that die is present.
///
/// Placements tied on grade-4 count are separated by `tie_break`; placements
/// tied even under that are broken in favor of the first found (row-major
/// order). On a wafer that uses only grade 1 both policies reduce to "most
/// good die", which is exactly the version-2 objective.
///
/// Returns `None` if the wafer has no placement at all that satisfies both
/// legality constraints (for example, a wafer whose present die area, once
/// inset by one die on every side, is smaller than the mask everywhere it
/// could sit).
pub fn find_best_region_with(map: &WaferMap, tie_break: TieBreak) -> Option<BestRegion> {
    let max_offset = BOARD_SIZE - MASK_SIZE;
    let mut best: Option<BestRegion> = None;
    for row in 0..=max_offset {
        for col in 0..=max_offset {
            let candidate = map.evaluate(row, col);
            if candidate.stats.overhang > 0 {
                continue;
            }
            if map.region_touches_wafer_edge(row, col) {
                continue;
            }
            // Strict `>` keeps the first placement in row-major order on ties.
            let better = match &best {
                None => true,
                Some(b) => candidate.stats.sort_key(tie_break) > b.stats.sort_key(tie_break),
            };
            if better {
                best = Some(candidate);
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
///
/// `tie_break` is recorded in the header because a result is not reproducible
/// without it: two policies can pick different regions from the same wafer, so
/// the artifact has to say which one produced it. [`WaferMap::header_tie_break`]
/// reads it back.
pub fn render_report(map: &WaferMap, region: &BestRegion, tie_break: TieBreak) -> String {
    let s = &region.stats;
    let breakdown: Vec<String> = Grade::BEST_FIRST
        .iter()
        .map(|g| format!("g{}={}", g.number(), s.grade(*g)))
        .collect();
    format!(
        "# yield_max 3  region=row{},col{} tiebreak={}\n\
         # good={} ({}) defect={} overhang={} sites={} yield={:.1}%\n\
         # {}\n{}",
        region.row,
        region.col,
        tie_break,
        s.good_total(),
        breakdown.join(" "),
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
