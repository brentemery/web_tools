use super::*;

/// The canonical fixture, shared with the CLI integration test rather than
/// re-typed, so the two cannot drift apart.
const SAMPLE: &str = include_str!("../../test_wafer.txt");

fn uniform(ch: char) -> String {
    let row: String = std::iter::repeat_n(ch, BOARD_SIZE).collect();
    (0..BOARD_SIZE).map(|_| row.as_str()).collect::<Vec<_>>().join("\n")
}

#[test]
fn parses_sample_wafer() {
    let map = WaferMap::parse(SAMPLE).expect("valid sample should parse");
    assert_eq!(map.get(0, 5), Die::Good);
    assert_eq!(map.get(0, 0), Die::Absent);
    assert_eq!(map.get(1, 3), Die::Defect);
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

#[test]
fn error_messages_are_one_based() {
    let msg = ParseError::InvalidChar {
        row: 0,
        col: 0,
        ch: '?',
    }
    .to_string();
    assert!(msg.contains("row 1, col 1"), "got: {msg}");
}

/// A blank line in the middle of the grid used to be silently dropped, which
/// shifted every subsequent row up by one and produced a confidently wrong
/// answer. It must be an error.
#[test]
fn rejects_blank_line_inside_grid() {
    let bad = SAMPLE.replacen('\n', "\n\n", 1);
    assert_eq!(
        WaferMap::parse(&bad).unwrap_err(),
        ParseError::WrongRowCount(BOARD_SIZE + 1)
    );
}

#[test]
fn tolerates_surrounding_blank_lines_and_crlf() {
    let padded = format!("\n\n{}\n\n", SAMPLE.trim());
    assert!(WaferMap::parse(&padded).is_ok());

    let crlf = SAMPLE.replace('\n', "\r\n");
    assert_eq!(WaferMap::parse(&crlf), WaferMap::parse(SAMPLE));
}

#[test]
fn tolerates_trailing_spaces() {
    let spaced = SAMPLE.trim().replace('\n', "   \n");
    assert!(WaferMap::parse(&spaced).is_ok());
}

#[test]
fn skips_comment_lines() {
    let commented = format!("# a header\n{}\n# a footer\n", SAMPLE.trim());
    assert_eq!(WaferMap::parse(&commented), WaferMap::parse(SAMPLE));
}

// Expected values computed independently (Python re-implementation of the
// same sliding-window search) against the sample wafer.
#[test]
fn finds_known_best_region_for_sample_wafer() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let best = find_best_region(&map);
    assert_eq!(
        best,
        BestRegion {
            row: 0,
            col: 5,
            stats: RegionStats {
                good: 63,
                defect: 29,
                overhang: 1,
            }
        }
    );
    assert_eq!(best.stats.sites(), mask_site_count());
}

#[test]
fn ranks_all_placements_best_first() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let ranked = rank_placements(&map);
    let span = BOARD_SIZE - MASK_SIZE + 1;
    assert_eq!(ranked.len(), span * span);
    assert!(ranked.windows(2).all(|w| w[0].stats.good >= w[1].stats.good));

    // Independently verified runner-up: it captures one fewer good die but
    // wastes no sites on overhang.
    assert_eq!((ranked[1].row, ranked[1].col, ranked[1].stats.good), (0, 4, 62));
    assert_eq!(ranked[1].stats.overhang, 0);
}

/// The docstring promises row-major first-wins on ties; an all-good wafer
/// makes every placement tie, which pins the behavior.
#[test]
fn breaks_ties_toward_first_in_row_major_order() {
    let map = WaferMap::parse(&uniform('1')).unwrap();
    let ranked = rank_placements(&map);
    assert!(ranked.iter().all(|p| p.stats.good == mask_site_count()));
    assert_eq!((ranked[0].row, ranked[0].col), (0, 0));
    assert_eq!((ranked[1].row, ranked[1].col), (0, 1));
}

#[test]
fn handles_all_good_and_all_absent_wafers() {
    let all_good = find_best_region(&WaferMap::parse(&uniform('1')).unwrap());
    assert_eq!(all_good.stats.good, mask_site_count());
    assert_eq!(all_good.stats.yield_fraction(), 1.0);

    let all_absent = find_best_region(&WaferMap::parse(&uniform('.')).unwrap());
    assert_eq!(all_absent.stats.good, 0);
    assert_eq!(all_absent.stats.overhang, mask_site_count());
    // No present die at all must not divide by zero.
    assert_eq!(all_absent.stats.yield_fraction(), 0.0);
}

#[test]
fn mark_region_rewrites_exactly_the_mask_footprint() {
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

            // Die state must survive marking; only the glyph changes.
            assert_eq!(marked_rows[r][c], map.get(r, c).to_char(inside_mask));
        }
    }
}

/// Every distinguishable state should appear in the sample's output, which is
/// the whole point of the version-2 alphabet.
#[test]
fn marked_output_uses_all_six_glyphs() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let marked = mark_region(&map, &find_best_region(&map));
    for ch in ['1', 'X', '.', 'Z', '*', '-'] {
        assert!(marked.contains(ch), "missing glyph {ch:?} in:\n{marked}");
    }
}

#[test]
fn report_header_carries_the_headline_numbers() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let report = render_report(&map, &find_best_region(&map));
    let header: Vec<&str> = report.lines().take(3).collect();
    assert_eq!(header[0], "# yield_max 2  region=row0,col5");
    assert_eq!(
        header[1],
        "# good=63 defect=29 overhang=1 sites=93 yield=68.5%"
    );
    assert!(header[2].contains("Z=good"));
}

/// The whole point of the lossless alphabet: our own output parses back to
/// the same wafer, and the region we chose is recoverable from it.
#[test]
fn output_round_trips_through_the_parser() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let best = find_best_region(&map);
    let report = render_report(&map, &best);

    let reparsed = WaferMap::parse(&report).expect("our own output must parse");
    for r in 0..BOARD_SIZE {
        for c in 0..BOARD_SIZE {
            assert_eq!(reparsed.get(r, c), map.get(r, c), "die state at ({r},{c})");
        }
    }
    assert_eq!(reparsed.marked_region(), Some(best));
    assert_eq!(find_best_region(&reparsed), best);
    // Idempotent: re-running on the marked file reproduces it exactly.
    assert_eq!(render_report(&reparsed, &find_best_region(&reparsed)), report);
}

#[test]
fn unmarked_and_bogus_maps_have_no_recoverable_region() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    assert_eq!(map.marked_region(), None);

    // A stray mark that matches no legal mask placement.
    let bogus = SAMPLE.replacen(".....1111111.....", "Z....1111111.....", 1);
    assert_eq!(WaferMap::parse(&bogus).unwrap().marked_region(), None);
}

#[test]
fn mask_matches_published_shape_and_site_count() {
    assert_eq!(mask_site_count(), 93);
    assert_eq!(MASK_TEMPLATE.len(), MASK_SIZE);
    assert!(MASK_TEMPLATE.iter().all(|r| r.len() == MASK_SIZE));
}
