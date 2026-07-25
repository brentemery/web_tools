//! Core solver for the Yield Max wafer-yield problem: given a 300mm wafer
//! map, find the highest-yielding placement of the fixed 200mm mask.

pub const BOARD_SIZE: usize = 17;
pub const MASK_SIZE: usize = 11;

const MASK_TEMPLATE: [&str; MASK_SIZE] = [
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

fn mask() -> [[bool; MASK_SIZE]; MASK_SIZE] {
    let mut grid = [[false; MASK_SIZE]; MASK_SIZE];
    for (r, row) in MASK_TEMPLATE.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            grid[r][c] = ch == 'O';
        }
    }
    grid
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaferMap {
    grid: Vec<Vec<char>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    WrongRowCount(usize),
    WrongRowLength { row: usize, len: usize },
    InvalidChar { row: usize, col: usize, ch: char },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::WrongRowCount(n) => {
                write!(f, "expected {BOARD_SIZE} rows, found {n}")
            }
            ParseError::WrongRowLength { row, len } => {
                write!(f, "row {row} has length {len}, expected {BOARD_SIZE}")
            }
            ParseError::InvalidChar { row, col, ch } => write!(
                f,
                "row {row}, col {col}: invalid character '{ch}' (expected '.', 'X', or '1')"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

impl WaferMap {
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let rows: Vec<&str> = input.lines().filter(|l| !l.trim().is_empty()).collect();
        if rows.len() != BOARD_SIZE {
            return Err(ParseError::WrongRowCount(rows.len()));
        }

        let mut grid = Vec::with_capacity(BOARD_SIZE);
        for (r, row) in rows.iter().enumerate() {
            let chars: Vec<char> = row.trim_end_matches('\r').chars().collect();
            if chars.len() != BOARD_SIZE {
                return Err(ParseError::WrongRowLength {
                    row: r,
                    len: chars.len(),
                });
            }
            for (c, &ch) in chars.iter().enumerate() {
                if ch != '.' && ch != 'X' && ch != '1' {
                    return Err(ParseError::InvalidChar { row: r, col: c, ch });
                }
            }
            grid.push(chars);
        }

        Ok(WaferMap { grid })
    }

    pub fn get(&self, row: usize, col: usize) -> char {
        self.grid[row][col]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BestRegion {
    pub row: usize,
    pub col: usize,
    pub good_die_count: usize,
}

/// Slides the 11x11 mask over every position where it fits fully inside the
/// 17x17 board and returns the placement covering the most good ('1') die.
/// Ties are broken in favor of the first placement found (row-major order).
pub fn find_best_region(map: &WaferMap) -> BestRegion {
    let mask = mask();
    let max_offset = BOARD_SIZE - MASK_SIZE;

    let mut best = BestRegion {
        row: 0,
        col: 0,
        good_die_count: 0,
    };
    let mut best_found = false;

    for row in 0..=max_offset {
        for col in 0..=max_offset {
            let mut good = 0;
            for dr in 0..MASK_SIZE {
                for dc in 0..MASK_SIZE {
                    if mask[dr][dc] && map.get(row + dr, col + dc) == '1' {
                        good += 1;
                    }
                }
            }
            if !best_found || good > best.good_die_count {
                best = BestRegion {
                    row,
                    col,
                    good_die_count: good,
                };
                best_found = true;
            }
        }
    }

    best
}

/// Renders a copy of `map` with every present die ('1' or 'X') inside the
/// winning mask footprint replaced with 'Z'. Non-present ('.') cells are left
/// untouched even when the mask overhangs them.
pub fn mark_region(map: &WaferMap, region: &BestRegion) -> String {
    let mask = mask();
    let mut grid = map.grid.clone();

    for dr in 0..MASK_SIZE {
        for dc in 0..MASK_SIZE {
            if mask[dr][dc] {
                let (r, c) = (region.row + dr, region.col + dc);
                if grid[r][c] == '1' || grid[r][c] == 'X' {
                    grid[r][c] = 'Z';
                }
            }
        }
    }

    grid.iter()
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
.....1111111.....
...XX111111XXX...
..X11X111X1X111..
.XXX1111111X111X.
.XXXX1111XX11X1X.
XXXXX1X111111XX1X
XXXXXX1111111X1XX
X1X111X1X1X11X1XX
XXX1XXXXXXX111XXX
1XX1XXX1X11X11XX1
XXXXXXXXX1111X11X
.11XXXXX11111X11.
.X1XXX1X11111XXX.
..1XX1XXXXX1111..
...XXXXXXXXXXX...
....X11111X1X....
.......XXXX......
";

    #[test]
    fn parses_sample_wafer() {
        let map = WaferMap::parse(SAMPLE).expect("valid sample should parse");
        assert_eq!(map.get(0, 5), '1');
        assert_eq!(map.get(0, 0), '.');
        assert_eq!(map.get(1, 3), 'X');
    }

    #[test]
    fn rejects_wrong_row_count() {
        let err = WaferMap::parse("111\n111\n").unwrap_err();
        assert_eq!(err, ParseError::WrongRowCount(2));
    }

    #[test]
    fn rejects_wrong_row_length() {
        let bad = SAMPLE.replacen(".....1111111.....", ".....1111111....", 1);
        let err = WaferMap::parse(&bad).unwrap_err();
        assert_eq!(
            err,
            ParseError::WrongRowLength {
                row: 0,
                len: BOARD_SIZE - 1
            }
        );
    }

    #[test]
    fn rejects_invalid_char() {
        let bad = SAMPLE.replacen('.', "?", 1);
        let err = WaferMap::parse(&bad).unwrap_err();
        assert_eq!(
            err,
            ParseError::InvalidChar {
                row: 0,
                col: 0,
                ch: '?'
            }
        );
    }

    // Expected value computed independently (Python re-implementation of the
    // same sliding-window search) against the sample wafer above.
    #[test]
    fn finds_known_best_region_for_sample_wafer() {
        let map = WaferMap::parse(SAMPLE).unwrap();
        let best = find_best_region(&map);
        assert_eq!(
            best,
            BestRegion {
                row: 0,
                col: 5,
                good_die_count: 63
            }
        );
    }

    #[test]
    fn mark_region_only_touches_mask_footprint() {
        let map = WaferMap::parse(SAMPLE).unwrap();
        let best = find_best_region(&map);
        let marked = mark_region(&map, &best);
        let marked_rows: Vec<Vec<char>> = marked.lines().map(|l| l.chars().collect()).collect();
        let mask = mask();

        for r in 0..BOARD_SIZE {
            for c in 0..BOARD_SIZE {
                let inside_mask = r >= best.row
                    && r < best.row + MASK_SIZE
                    && c >= best.col
                    && c < best.col + MASK_SIZE
                    && mask[r - best.row][c - best.col];

                let original = map.get(r, c);
                let marked_cell = marked_rows[r][c];

                if inside_mask && (original == '1' || original == 'X') {
                    assert_eq!(marked_cell, 'Z');
                } else {
                    assert_eq!(marked_cell, original);
                }
            }
        }
    }
}
