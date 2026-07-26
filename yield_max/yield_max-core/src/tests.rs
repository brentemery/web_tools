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

/// The docstring promises row-major first-wins on ties; an all-good wafer
/// makes every placement tie, which pins the behavior.
#[test]
fn breaks_ties_toward_first_in_row_major_order() {
    let map = WaferMap::parse(&uniform('1')).unwrap();
    let best = find_best_region(&map);
    assert_eq!(best.stats.good, mask_site_count());
    assert_eq!((best.row, best.col), (0, 0));
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

// ---------------------------------------------------------------------------
// Fixture files in testdata/
//
// Each valid fixture carries its own expected answer in a `# expect:` header,
// so the assertion lives next to the data it describes and a new fixture is
// picked up just by dropping the file in. The expected values were computed by
// an independent Python implementation of the search, not recorded from this
// code, so these tests can catch a regression rather than merely pinning
// current behavior.
// ---------------------------------------------------------------------------

/// Reads `key=value` pairs off the fixture's `# expect:` line.
fn expectations(text: &str) -> Option<BestRegion> {
    let line = text
        .lines()
        .find(|l| l.starts_with("# expect:"))?
        .trim_start_matches("# expect:");
    let mut fields = std::collections::HashMap::new();
    for pair in line.split_whitespace() {
        let (k, v) = pair.split_once('=')?;
        fields.insert(k, v.parse::<usize>().ok()?);
    }
    Some(BestRegion {
        row: *fields.get("row")?,
        col: *fields.get("col")?,
        stats: RegionStats {
            good: *fields.get("good")?,
            defect: *fields.get("defect")?,
            overhang: *fields.get("overhang")?,
        },
    })
}

/// Every `.txt` directly inside testdata/ (excluding the invalid/ subdirectory).
fn valid_fixtures() -> Vec<(String, String)> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata");
    let mut out: Vec<(String, String)> = std::fs::read_dir(dir)
        .expect("testdata/ must exist")
        .filter_map(|e| {
            let path = e.ok()?.path();
            if path.extension()? != "txt" {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            Some((name, std::fs::read_to_string(&path).ok()?))
        })
        .collect();
    out.sort();
    out
}

#[test]
fn every_valid_fixture_parses_and_matches_its_expected_result() {
    let fixtures = valid_fixtures();
    // Guard against the glob silently matching nothing and the test passing
    // vacuously if testdata/ is ever moved.
    assert!(
        fixtures.len() >= 10,
        "expected the fixture set, found {}",
        fixtures.len()
    );

    let mut checked = 0;
    for (name, text) in &fixtures {
        let map = WaferMap::parse(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let best = find_best_region(&map);
        assert_eq!(
            best.stats.sites(),
            mask_site_count(),
            "{name}: a placement always covers the same number of sites"
        );
        if let Some(expected) = expectations(text) {
            assert_eq!(best, expected, "{name}: result disagrees with its header");
            checked += 1;
        }
    }
    assert!(checked >= 9, "only {checked} fixtures carried expectations");
}

/// The marked fixture is the tool's own output, so re-running on it must be a
/// no-op -- the property that makes the format safely re-enterable.
#[test]
fn marked_fixture_round_trips_and_reveals_its_region() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../testdata/marked_roundtrip.txt"
    ))
    .unwrap();
    let map = WaferMap::parse(&text).expect("marked output must parse");

    let recovered = map.marked_region().expect("region should be recoverable");
    assert_eq!((recovered.row, recovered.col), (0, 5));
    assert_eq!(recovered.stats.good, 63);

    let best = find_best_region(&map);
    assert_eq!(best, recovered, "re-solving must find the same region");
    // Byte-identical below the fixture's leading note lines.
    let report = render_report(&map, &best);
    assert!(
        text.ends_with(&report),
        "re-rendering must reproduce the fixture body"
    );
}

#[test]
fn every_invalid_fixture_is_rejected() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/invalid");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).expect("testdata/invalid must exist") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
        let parsed = WaferMap::parse(&text);

        if name == "stray_mark" {
            // This one is well-formed; the defect is semantic, not syntactic.
            let map = parsed.unwrap_or_else(|e| panic!("stray_mark should parse: {e}"));
            assert_eq!(
                map.marked_region(),
                None,
                "marks matching no legal placement must not yield a region"
            );
        } else {
            assert!(parsed.is_err(), "{name} should have been rejected");
        }
        seen += 1;
    }
    assert!(seen >= 8, "only found {seen} invalid fixtures");
}

/// CRLF endings, trailing spaces, and surrounding blank lines are all
/// tolerated, and must not change the answer.
#[test]
fn messy_whitespace_fixture_matches_the_clean_one() {
    let messy = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../testdata/messy_whitespace.txt"
    ))
    .unwrap();
    assert!(messy.contains('\r'), "fixture should exercise CRLF");
    assert_eq!(
        find_best_region(&WaferMap::parse(&messy).unwrap()),
        find_best_region(&WaferMap::parse(SAMPLE).unwrap())
    );
}
