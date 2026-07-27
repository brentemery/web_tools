use super::*;

/// The canonical fixture, shared with the CLI integration test rather than
/// re-typed, so the two cannot drift apart.
const SAMPLE: &str = include_str!("../../test_wafer.txt");

/// Just the 17 grid rows of [`SAMPLE`], without whatever header text the
/// fixture file happens to carry above them. Tests that mutate a specific
/// character by position use this instead of `SAMPLE` directly, so they
/// don't depend on what (if anything) precedes the grid in the fixture file.
fn grid_only() -> String {
    let lines: Vec<&str> = SAMPLE.lines().collect();
    lines[lines.len() - BOARD_SIZE..].join("\n")
}

fn uniform(ch: char) -> String {
    let row: String = std::iter::repeat_n(ch, BOARD_SIZE).collect();
    (0..BOARD_SIZE)
        .map(|_| row.as_str())
        .collect::<Vec<_>>()
        .join("\n")
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
    // Full-width rows, so they aren't swallowed by the header-skipping
    // heuristic (see `skips_header_text_above_the_grid`), which only treats
    // narrower leading lines as header text.
    let row = ".....1111111.....";
    let err = WaferMap::parse(&format!("{row}\n{row}\n")).unwrap_err();
    assert_eq!(err, ParseError::WrongRowCount(2));
}

#[test]
fn rejects_wrong_row_length() {
    // Row 0 is deliberately left alone: shortening the *first* row is
    // indistinguishable from a free-text header line, so it is dropped as
    // header text (see `skips_header_text_above_the_grid`) rather than
    // reported as a malformed row. A row further in still gets a precise
    // error, which is what this test pins down.
    let bad = SAMPLE.replacen("XXXXX1X111111XX1X", "XXXXX1X111111XX1", 1);
    let err = WaferMap::parse(&bad).unwrap_err();
    assert_eq!(
        err,
        ParseError::WrongRowLength {
            row: 5,
            len: BOARD_SIZE - 1
        }
    );
}

#[test]
fn rejects_invalid_char() {
    let bad = grid_only().replacen('.', "?", 1);
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
    let bad = grid_only().replacen('\n', "\n\n", 1);
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

/// Some source systems prepend free-text metadata above the grid with no `#`
/// marker at all. Any leading line that isn't 17 characters wide is dropped
/// as header text; multiple lines, blank lines, and `#` comments in the mix
/// are all tolerated together.
#[test]
fn skips_header_text_above_the_grid() {
    let headered = format!(
        "Wafer Map Report\nLot: ACME-042   Slot: 07\nOperator: J. Diaz   Date: 2026-07-20\n\n{}",
        SAMPLE.trim()
    );
    assert_eq!(WaferMap::parse(&headered), WaferMap::parse(SAMPLE));
}

/// A header line that happens to be exactly 17 characters wide is left for
/// normal validation rather than assumed to be header text, since we cannot
/// tell it apart from a genuinely malformed first grid row.
#[test]
fn seventeen_character_header_line_is_treated_as_a_grid_row() {
    let headered = SAMPLE.replacen(".....1111111.....", "Lot: ACME-042 ###", 1);
    assert!(matches!(
        WaferMap::parse(&headered).unwrap_err(),
        ParseError::InvalidChar { row: 0, .. }
    ));
}

// Expected values computed independently (Python re-implementation of the
// same sliding-window search) against the sample wafer.
#[test]
fn finds_known_best_region_for_sample_wafer() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let best = find_best_region(&map).unwrap();
    assert_eq!(
        best,
        BestRegion {
            row: 2,
            col: 4,
            stats: RegionStats {
                good: 57,
                defect: 36,
                overhang: 0,
            }
        }
    );
    assert_eq!(best.stats.sites(), mask_site_count());
}

/// The core invariant this file format enforces: a 200mm region that hangs
/// off the wafer edge, or that covers a die that itself sits on the wafer's
/// edge, is not legal -- even when it covers more good die than the best
/// legal alternative. (0,5) on the sample wafer covers 63 good die but
/// carries 1 overhang site; (0,4) covers 62 good die with zero overhang but
/// still fails the edge rule, since row 0 is always the wafer's true edge.
/// Both lose to (2,4), the actual winner.
#[test]
fn overhang_placement_loses_to_a_lower_good_overhang_free_one() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let with_overhang = map.evaluate(0, 5);
    assert_eq!(
        (with_overhang.stats.good, with_overhang.stats.overhang),
        (63, 1)
    );

    let best = find_best_region(&map).unwrap();
    assert_eq!((best.row, best.col), (2, 4));
    assert!(best.stats.good < with_overhang.stats.good);
    assert_eq!(best.stats.overhang, 0);
}

/// The docstring promises row-major first-wins on ties; an all-good wafer
/// makes every placement tie, which pins the behavior. (0,0) is never in
/// contention -- it always touches the wafer's true edge -- so the first
/// legal placement in row-major order is (1,1).
#[test]
fn breaks_ties_toward_first_in_row_major_order() {
    let map = WaferMap::parse(&uniform('1')).unwrap();
    let best = find_best_region(&map).unwrap();
    assert_eq!(best.stats.good, mask_site_count());
    assert_eq!((best.row, best.col), (1, 1));
}

#[test]
fn handles_all_good_and_all_absent_wafers() {
    let all_good = find_best_region(&WaferMap::parse(&uniform('1')).unwrap()).unwrap();
    assert_eq!(all_good.stats.good, mask_site_count());
    assert_eq!(all_good.stats.yield_fraction(), 1.0);

    // An all-absent wafer has no placement anywhere that avoids overhang, so
    // there is no legal 200mm region at all.
    let all_absent = WaferMap::parse(&uniform('.')).unwrap();
    assert_eq!(find_best_region(&all_absent), None);
    // The zero-overhang guard on yield_fraction still matters for a raw,
    // unconstrained placement (e.g. one recovered from marked_region()).
    let region = all_absent.evaluate(0, 0);
    assert_eq!(region.stats.overhang, mask_site_count());
    assert_eq!(region.stats.yield_fraction(), 0.0);
}

#[test]
fn mark_region_rewrites_exactly_the_mask_footprint() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let best = find_best_region(&map).unwrap();
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

/// Every distinguishable state `mark_region` can produce should appear in
/// its output, which is the whole point of the version-2 alphabet. The
/// region here is a raw placement chosen directly via `evaluate`, not
/// `find_best_region` (which now excludes overhang placements as illegal),
/// specifically because it carries overhang and so exercises the `-` glyph.
#[test]
fn marked_output_uses_all_six_glyphs() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let region = map.evaluate(0, 5);
    assert_eq!(
        region.stats.overhang, 1,
        "fixture assumption: (0,5) has overhang"
    );
    let marked = mark_region(&map, &region);
    for ch in ['1', 'X', '.', 'Z', '*', '-'] {
        assert!(marked.contains(ch), "missing glyph {ch:?} in:\n{marked}");
    }
}

#[test]
fn report_header_carries_the_headline_numbers() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let report = render_report(&map, &find_best_region(&map).unwrap());
    let header: Vec<&str> = report.lines().take(3).collect();
    assert_eq!(header[0], "# yield_max 2  region=row2,col4");
    assert_eq!(
        header[1],
        "# good=57 defect=36 overhang=0 sites=93 yield=61.3%"
    );
    assert!(header[2].contains("Z=good"));
}

/// The whole point of the lossless alphabet: our own output parses back to
/// the same wafer, and the region we chose is recoverable from it.
#[test]
fn output_round_trips_through_the_parser() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let best = find_best_region(&map).unwrap();
    let report = render_report(&map, &best);

    let reparsed = WaferMap::parse(&report).expect("our own output must parse");
    for r in 0..BOARD_SIZE {
        for c in 0..BOARD_SIZE {
            assert_eq!(reparsed.get(r, c), map.get(r, c), "die state at ({r},{c})");
        }
    }
    assert_eq!(reparsed.marked_region(), Some(best));
    assert_eq!(find_best_region(&reparsed), Some(best));
    // Idempotent: re-running on the marked file reproduces it exactly.
    assert_eq!(
        render_report(&reparsed, &find_best_region(&reparsed).unwrap()),
        report
    );
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

/// What a fixture's `# expect:` line says `find_best_region` should return:
/// either a specific placement, or `None` because no legal (overhang-free)
/// placement exists anywhere on the wafer.
enum Expectation {
    Region(BestRegion),
    NoRegion,
}

/// Reads the fixture's `# expect:` line, either `none` or `key=value` pairs.
fn expectations(text: &str) -> Option<Expectation> {
    let line = text
        .lines()
        .find(|l| l.starts_with("# expect:"))?
        .trim_start_matches("# expect:")
        .trim();
    if line == "none" {
        return Some(Expectation::NoRegion);
    }
    let mut fields = std::collections::HashMap::new();
    for pair in line.split_whitespace() {
        let (k, v) = pair.split_once('=')?;
        fields.insert(k, v.parse::<usize>().ok()?);
    }
    Some(Expectation::Region(BestRegion {
        row: *fields.get("row")?,
        col: *fields.get("col")?,
        stats: RegionStats {
            good: *fields.get("good")?,
            defect: *fields.get("defect")?,
            overhang: *fields.get("overhang")?,
        },
    }))
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
        if let Some(region) = &best {
            assert_eq!(
                region.stats.sites(),
                mask_site_count(),
                "{name}: a placement always covers the same number of sites"
            );
            assert_eq!(
                region.stats.overhang, 0,
                "{name}: a legal region can never carry overhang"
            );
        }
        match expectations(text) {
            Some(Expectation::Region(expected)) => {
                assert_eq!(
                    best,
                    Some(expected),
                    "{name}: result disagrees with its header"
                );
                checked += 1;
            }
            Some(Expectation::NoRegion) => {
                assert_eq!(
                    best, None,
                    "{name}: expected no legal region, but found one"
                );
                checked += 1;
            }
            None => {}
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
    assert_eq!((recovered.row, recovered.col), (2, 4));
    assert_eq!(recovered.stats.good, 57);

    let best = find_best_region(&map).unwrap();
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

// ---------------------------------------------------------------------------
// Input validation. These pin down what we accept and, more importantly, that
// what we reject fails loudly rather than being silently reinterpreted.
// ---------------------------------------------------------------------------

#[test]
fn rejects_oversized_input() {
    let huge = "1".repeat(MAX_INPUT_BYTES + 1);
    assert_eq!(
        WaferMap::parse(&huge).unwrap_err(),
        ParseError::TooLarge {
            bytes: MAX_INPUT_BYTES + 1
        }
    );
    // The limit is generous enough for a real map plus a comment header.
    assert!(SAMPLE.len() < MAX_INPUT_BYTES / 10);
}

/// A BOM is invisible in most editors, so reporting "row 1 has length 18"
/// about a row the user sees as 17 characters would be actively misleading.
#[test]
fn strips_utf8_bom() {
    let with_bom = format!("\u{feff}{SAMPLE}");
    assert_eq!(WaferMap::parse(&with_bom), WaferMap::parse(SAMPLE));
}

#[test]
fn rejects_invisible_characters_with_a_legible_message() {
    // Each of these is either invisible or indistinguishable from a legal
    // character at a glance, so the message must name it.
    for (ch, expected) in [
        ('\t', "tab"),
        ('\u{a0}', "non-breaking space"),
        ('\u{200b}', "zero-width space"),
        (' ', "space"),
    ] {
        let bad = grid_only().replacen('.', &ch.to_string(), 1);
        let msg = WaferMap::parse(&bad).unwrap_err().to_string();
        assert!(
            msg.contains(expected) && msg.contains("U+"),
            "{ch:?} produced an unhelpful message: {msg}"
        );
    }
}

/// A homoglyph: fullwidth '１' looks like a good die but is not one.
#[test]
fn rejects_non_ascii_lookalikes() {
    let bad = grid_only().replacen('1', "\u{ff11}", 1);
    let msg = WaferMap::parse(&bad).unwrap_err().to_string();
    assert!(msg.contains("U+FF11"), "got: {msg}");
}

#[test]
fn rejects_empty_and_contentless_input() {
    for input in ["", "   \n\n  \n", "# only a comment\n# and another\n"] {
        assert_eq!(
            WaferMap::parse(input).unwrap_err(),
            ParseError::WrongRowCount(0),
            "input {input:?} should be rejected"
        );
    }
}

/// Comments are a header/footer convention. One interleaved with the grid
/// suggests the file was assembled wrongly, so treat it like a blank line
/// there and refuse rather than quietly stitching the halves together.
#[test]
fn rejects_comment_inside_the_grid() {
    let rows: Vec<&str> = SAMPLE.trim().lines().collect();
    let spliced = format!("{}\n# note\n{}", rows[..8].join("\n"), rows[8..].join("\n"));
    assert_eq!(
        WaferMap::parse(&spliced).unwrap_err(),
        ParseError::WrongRowCount(BOARD_SIZE + 1)
    );
}

/// Marks that match no legal placement are overwritten by the next report.
/// That is acceptable, but callers must be able to warn first.
#[test]
fn flags_marks_that_match_no_legal_placement() {
    let clean = WaferMap::parse(SAMPLE).unwrap();
    assert!(!clean.has_marks());
    assert!(!clean.has_inconsistent_marks());

    let stray = WaferMap::parse(&grid_only().replacen('.', "Z", 1)).unwrap();
    assert!(stray.has_marks());
    assert!(
        stray.has_inconsistent_marks(),
        "a lone Z matches no placement and must be flagged"
    );

    // Our own output is marked, but consistently so: no warning.
    let map = WaferMap::parse(SAMPLE).unwrap();
    let report = render_report(&map, &find_best_region(&map).unwrap());
    let remarked = WaferMap::parse(&report).unwrap();
    assert!(remarked.has_marks());
    assert!(!remarked.has_inconsistent_marks());
}

/// CR-only (classic Mac) endings collapse to a single line. We cannot know
/// the user's intent, but we must not silently misread the file.
#[test]
fn rejects_cr_only_line_endings() {
    let cr = SAMPLE.trim().replace('\n', "\r");
    assert!(matches!(
        WaferMap::parse(&cr).unwrap_err(),
        ParseError::WrongRowCount(_) | ParseError::WrongRowLength { .. }
    ));
}
