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

/// [`SAMPLE`]'s grid with its good die cycled through all four grades, so a
/// test needing graded input doesn't have to hand-draw a wafer. The exact
/// assignment is arbitrary but deterministic; tests using it assert only
/// properties that hold for any assignment.
fn graded_sample() -> String {
    let mut n = 0;
    grid_only()
        .chars()
        .map(|ch| {
            if ch == '1' {
                n += 1;
                char::from(b'1' + (n % 4) as u8)
            } else {
                ch
            }
        })
        .collect()
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
    assert_eq!(map.get(0, 5), Die::Good(Grade::G1));
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
            stats: RegionStats::new([57, 0, 0, 0], 36, 0),
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
        (
            with_overhang.stats.good_total(),
            with_overhang.stats.overhang
        ),
        (63, 1)
    );

    let best = find_best_region(&map).unwrap();
    assert_eq!((best.row, best.col), (2, 4));
    assert!(best.stats.good_total() < with_overhang.stats.good_total());
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
    assert_eq!(best.stats.good_total(), mask_site_count());
    assert_eq!((best.row, best.col), (1, 1));
}

#[test]
fn handles_all_good_and_all_absent_wafers() {
    let all_good = find_best_region(&WaferMap::parse(&uniform('1')).unwrap()).unwrap();
    assert_eq!(all_good.stats.good_total(), mask_site_count());
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
/// its output, which is the whole point of the version-3 alphabet. The
/// region here is a raw placement chosen directly via `evaluate`, not
/// `find_best_region` (which now excludes overhang placements as illegal),
/// specifically because it carries overhang and so exercises the `-` glyph.
/// The wafer is a graded one so all four in-region good glyphs appear.
#[test]
fn marked_output_uses_every_glyph_in_the_alphabet() {
    let map = WaferMap::parse(&graded_sample()).unwrap();
    let region = map.evaluate(0, 5);
    assert_eq!(
        region.stats.overhang, 1,
        "fixture assumption: (0,5) has overhang"
    );
    let marked = mark_region(&map, &region);
    for ch in ['1', '2', '3', '4', 'X', '.', 'A', 'B', 'C', 'D', '*', '-'] {
        assert!(marked.contains(ch), "missing glyph {ch:?} in:\n{marked}");
    }
    // `Z` is accepted on input but must never be emitted.
    assert!(
        !marked.contains(LEGACY_IN_REGION_GOOD),
        "v2's Z must not be emitted:\n{marked}"
    );
}

#[test]
fn report_header_carries_the_headline_numbers() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let report = render_report(&map, &find_best_region(&map).unwrap(), TieBreak::Grade);
    let header: Vec<&str> = report.lines().take(3).collect();
    assert_eq!(header[0], "# yield_max 3  region=row2,col4 tiebreak=grade");
    assert_eq!(
        header[1],
        "# good=57 (g4=0 g3=0 g2=0 g1=57) defect=36 overhang=0 sites=93 yield=61.3%"
    );
    assert!(header[2].contains("D=good4"));
}

/// The whole point of the lossless alphabet: our own output parses back to
/// the same wafer, and the region we chose is recoverable from it.
#[test]
fn output_round_trips_through_the_parser() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let best = find_best_region(&map).unwrap();
    let report = render_report(&map, &best, TieBreak::Grade);

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
        render_report(
            &reparsed,
            &find_best_region(&reparsed).unwrap(),
            TieBreak::Grade
        ),
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

// ---------------------------------------------------------------------------
// Graded good die and the tie-break policy.
// ---------------------------------------------------------------------------

/// The alphabet is only lossless if `to_char` and `from_char` are exact
/// inverses over every state, in both region positions. Table-driven so a new
/// glyph cannot be added to one direction and forgotten in the other.
#[test]
fn every_glyph_round_trips_through_the_die_alphabet() {
    let states = [
        Die::Good(Grade::G1),
        Die::Good(Grade::G2),
        Die::Good(Grade::G3),
        Die::Good(Grade::G4),
        Die::Defect,
        Die::Absent,
    ];

    let mut glyphs = std::collections::HashSet::new();
    for die in states {
        for in_region in [false, true] {
            let ch = die.to_char(in_region);
            assert!(glyphs.insert(ch), "glyph {ch:?} is used for two states");
            assert_eq!(
                Die::from_char(ch),
                Some((die, in_region)),
                "glyph {ch:?} did not round-trip"
            );
        }
    }
    assert_eq!(
        glyphs.len(),
        12,
        "expected 4 grades x 2 + defect/absent x 2"
    );

    // The documented spellings, pinned so a refactor of the derivation from
    // grade number to character cannot silently renumber the alphabet.
    assert_eq!(Die::Good(Grade::G4).to_char(false), '4');
    assert_eq!(Die::Good(Grade::G4).to_char(true), 'D');
    assert_eq!(Die::Good(Grade::G1).to_char(true), 'A');
}

/// Version 2 marked every in-region good die `Z`. Such a file must still parse
/// -- as an in-region grade-1 die -- so an old report is valid input, but `Z`
/// is never written back out.
#[test]
fn legacy_z_parses_as_an_in_region_grade_1_die() {
    assert_eq!(
        Die::from_char(LEGACY_IN_REGION_GOOD),
        Some((Die::Good(Grade::G1), true))
    );
    assert_eq!(Die::Good(Grade::G1).to_char(true), 'A');

    // A whole v2 report parses, and re-rendering it yields the v3 alphabet
    // with the same region and the same die states.
    let map = WaferMap::parse(SAMPLE).unwrap();
    let best = find_best_region(&map).unwrap();
    let v2_report = render_report(&map, &best, TieBreak::Grade).replace('A', "Z");
    let reparsed = WaferMap::parse(&v2_report).expect("a v2 report must parse");
    assert_eq!(
        reparsed.marked_region().map(|r| (r.row, r.col)),
        Some((2, 4))
    );
    let rerendered = render_report(
        &reparsed,
        &find_best_region(&reparsed).unwrap(),
        TieBreak::Grade,
    );
    assert!(!rerendered.contains(LEGACY_IN_REGION_GOOD));
    assert!(rerendered.contains('A'));
}

/// `1`..`4` are all good die; only the grade differs. `good_total` and
/// `yield_fraction` count them all, which is what keeps the header's `good=`
/// and `yield=` fields meaning what they meant in version 2.
#[test]
fn all_four_grades_are_good_die() {
    for (ch, grade) in [
        ('1', Grade::G1),
        ('2', Grade::G2),
        ('3', Grade::G3),
        ('4', Grade::G4),
    ] {
        let map = WaferMap::parse(&uniform(ch)).unwrap();
        assert_eq!(map.get(8, 8), Die::Good(grade));
        assert!(map.get(8, 8).is_good());

        let best = find_best_region(&map).unwrap();
        assert_eq!(best.stats.good_total(), mask_site_count());
        assert_eq!(best.stats.grade(grade), mask_site_count());
        assert_eq!(best.stats.yield_fraction(), 1.0);
        // An all-one-grade wafer is a total tie, so the row-major first-wins
        // rule applies exactly as it did for an all-`1` wafer.
        assert_eq!((best.row, best.col), (1, 1));
    }
}

/// The objective: more grade-4 die wins, even against a placement carrying
/// far more good die overall, and under either tie-break policy.
#[test]
fn grade_4_count_outranks_total_good_die() {
    let few_g4 = RegionStats::new([90, 0, 0, 1], 2, 0);
    let many_good = RegionStats::new([93, 0, 0, 0], 0, 0);
    for tb in TieBreak::ALL {
        assert!(
            few_g4.sort_key(tb) > many_good.sort_key(tb),
            "{tb}: one grade-4 die must outrank three more good die"
        );
    }
    assert!(many_good.good_total() > few_g4.good_total());
}

/// Where the two policies part company: equal grade-4 counts, and one side
/// has better remaining grades while the other has more good die in total.
#[test]
fn tie_break_policies_disagree_only_below_the_grade_4_count() {
    // Same n4; `better_grades` has 2 grade-3, `more_total` has 3 grade-1.
    let better_grades = RegionStats::new([0, 0, 2, 5], 0, 0);
    let more_total = RegionStats::new([3, 0, 0, 5], 0, 0);
    assert_eq!(better_grades.grade(Grade::G4), more_total.grade(Grade::G4));
    assert!(more_total.good_total() > better_grades.good_total());

    assert!(
        better_grades.sort_key(TieBreak::Grade) > more_total.sort_key(TieBreak::Grade),
        "grade policy must prefer the better remaining grades"
    );
    assert!(
        more_total.sort_key(TieBreak::Total) > better_grades.sort_key(TieBreak::Total),
        "total policy must prefer the larger good count"
    );
}

/// `Total` still orders placements that tie on both n4 and the total, so the
/// result never depends on which of two "equal" candidates was visited first
/// for reasons the caller can't see.
#[test]
fn total_policy_stays_a_total_order_below_the_good_count() {
    let a = RegionStats::new([1, 0, 2, 5], 0, 0);
    let b = RegionStats::new([0, 2, 1, 5], 0, 0);
    assert_eq!(a.good_total(), b.good_total());
    assert_eq!(a.grade(Grade::G4), b.grade(Grade::G4));
    assert!(a.sort_key(TieBreak::Total) > b.sort_key(TieBreak::Total));
    assert_ne!(a.sort_key(TieBreak::Total), b.sort_key(TieBreak::Total));
}

/// On a wafer with no grade above 1, both keys reduce to the good-die count --
/// the version-2 objective. This is the property that lets every pre-existing
/// fixture keep its recorded answer.
#[test]
fn both_policies_reduce_to_good_count_without_grades() {
    for (good, other) in [(57usize, 40usize), (0, 1), (93, 92)] {
        let more = RegionStats::new([good, 0, 0, 0], 0, 0);
        let less = RegionStats::new([other, 0, 0, 0], 0, 0);
        for tb in TieBreak::ALL {
            assert_eq!(
                more.sort_key(tb) > less.sort_key(tb),
                good > other,
                "{tb}: ungraded comparison must follow the good count"
            );
        }
    }
}

#[test]
fn tie_break_names_round_trip_and_junk_is_rejected() {
    for tb in TieBreak::ALL {
        assert_eq!(tb.as_str().parse::<TieBreak>(), Ok(tb));
        assert_eq!(tb.to_string(), tb.as_str());
    }
    assert_eq!(TieBreak::default(), TieBreak::Grade);

    // Unrecognized spellings must fail loudly and name the alternatives,
    // rather than falling back to the default and silently answering a
    // different question than the one asked.
    for bad in ["", "Grade", "totals", "g4"] {
        let err = bad.parse::<TieBreak>().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("grade") && msg.contains("total"), "got: {msg}");
    }
}

/// A report records the policy that produced it, and the parser hands it back,
/// so re-running on a report can reproduce it instead of quietly switching to
/// the default.
#[test]
fn report_header_records_the_tie_break_and_the_parser_recovers_it() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    for tb in TieBreak::ALL {
        let report = render_report(&map, &find_best_region_with(&map, tb).unwrap(), tb);
        assert!(
            report.contains(&format!("tiebreak={tb}")),
            "header must name the policy: {report}"
        );
        let reparsed = WaferMap::parse(&report).unwrap();
        assert_eq!(reparsed.header_tie_break(), Some(&Ok(tb)));
    }

    // No header, no opinion -- the caller falls back to its own default.
    assert_eq!(WaferMap::parse(SAMPLE).unwrap().header_tie_break(), None);

    // A header naming something we don't understand is surfaced as an error,
    // not treated as absent: the file claims a policy we can't reproduce.
    let bogus = format!(
        "# yield_max 3  region=row2,col4 tiebreak=sideways\n{}",
        SAMPLE
    );
    let parsed = WaferMap::parse(&bogus).unwrap();
    assert!(matches!(parsed.header_tie_break(), Some(Err(_))));
}

/// The report's grade breakdown must add up to its own `good=` total, and
/// match the per-grade counts of the region it describes.
#[test]
fn report_header_breaks_good_die_down_by_grade() {
    let map = WaferMap::parse(&graded_sample()).unwrap();
    let best = find_best_region(&map).unwrap();
    let report = render_report(&map, &best, TieBreak::Grade);
    let stats_line = report.lines().nth(1).unwrap();

    assert!(
        stats_line.contains(&format!("good={} (", best.stats.good_total())),
        "got: {stats_line}"
    );
    for g in Grade::BEST_FIRST {
        assert!(
            stats_line.contains(&format!("g{}={}", g.number(), best.stats.grade(g))),
            "missing grade {} in: {stats_line}",
            g.number()
        );
    }
    assert_eq!(
        best.stats.by_grade().iter().sum::<usize>(),
        best.stats.good_total()
    );
}

/// End to end on a real graded wafer: the winner must be the placement with
/// the most grade-4 die, and no legal placement may beat it on that count.
#[test]
fn winner_maximizes_grade_4_over_all_legal_placements() {
    let map = WaferMap::parse(&graded_sample()).unwrap();
    for tb in TieBreak::ALL {
        let best = find_best_region_with(&map, tb).unwrap();
        for row in 0..=(BOARD_SIZE - MASK_SIZE) {
            for col in 0..=(BOARD_SIZE - MASK_SIZE) {
                let c = map.evaluate(row, col);
                if c.stats.overhang > 0 || map.region_touches_wafer_edge(row, col) {
                    continue;
                }
                assert!(
                    c.stats.grade(Grade::G4) <= best.stats.grade(Grade::G4),
                    "{tb}: ({row},{col}) has more grade-4 die than the winner"
                );
            }
        }
    }
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

/// Reads a fixture's expectation line, either `none` or `key=value` pairs.
///
/// `# expect:` gives the answer under the default policy
/// ([`TieBreak::Grade`]); the optional `# expect-total:` gives it under
/// [`TieBreak::Total`]. Per-grade counts are written `g4=` .. `g1=`; a fixture
/// that writes only `good=` is taken to be all grade 1, which is why every
/// version-2 fixture needed no edit.
fn expectation_for(text: &str, tie_break: TieBreak) -> Option<Expectation> {
    let key = match tie_break {
        TieBreak::Grade => "# expect:",
        TieBreak::Total => "# expect-total:",
    };
    let line = text
        .lines()
        .find(|l| l.starts_with(key))?
        .trim_start_matches(key)
        .trim();
    if line == "none" {
        return Some(Expectation::NoRegion);
    }
    let mut fields = std::collections::HashMap::new();
    for pair in line.split_whitespace() {
        let (k, v) = pair.split_once('=')?;
        fields.insert(k, v.parse::<usize>().ok()?);
    }

    // Per-grade counts if given, else the whole `good=` total as grade 1.
    let mut good = [0usize; GRADES];
    let graded = Grade::BEST_FIRST
        .iter()
        .any(|g| fields.contains_key(format!("g{}", g.number()).as_str()));
    if graded {
        for g in Grade::BEST_FIRST {
            good[g.number() as usize - 1] = *fields.get(format!("g{}", g.number()).as_str())?;
        }
        // The total, when also given, must agree -- a fixture header that
        // contradicts itself is a bug in the fixture.
        if let Some(total) = fields.get("good") {
            assert_eq!(
                *total,
                good.iter().sum::<usize>(),
                "fixture header: good= disagrees with the per-grade counts"
            );
        }
    } else {
        good[0] = *fields.get("good")?;
    }

    Some(Expectation::Region(BestRegion {
        row: *fields.get("row")?,
        col: *fields.get("col")?,
        stats: RegionStats::new(good, *fields.get("defect")?, *fields.get("overhang")?),
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
        // A fixture is checked under every policy whose expectation it
        // declares; `# expect-total:` is optional and only appears where the
        // two policies actually differ.
        for tie_break in TieBreak::ALL {
            let best = find_best_region_with(&map, tie_break);
            if let Some(region) = &best {
                assert_eq!(
                    region.stats.sites(),
                    mask_site_count(),
                    "{name}/{tie_break}: a placement always covers the same number of sites"
                );
                assert_eq!(
                    region.stats.overhang, 0,
                    "{name}/{tie_break}: a legal region can never carry overhang"
                );
            }
            match expectation_for(text, tie_break) {
                Some(Expectation::Region(expected)) => {
                    assert_eq!(
                        best,
                        Some(expected),
                        "{name}/{tie_break}: result disagrees with its header"
                    );
                    checked += 1;
                }
                Some(Expectation::NoRegion) => {
                    assert_eq!(
                        best, None,
                        "{name}/{tie_break}: expected no legal region, but found one"
                    );
                    checked += 1;
                }
                None => {}
            }
        }
    }
    assert!(checked >= 9, "only {checked} fixtures carried expectations");
}

/// The backward-compatibility proof. Every fixture that predates grades uses
/// only grade-1 good die, and on such a wafer both tie-break policies must
/// reduce to the version-2 objective ("most good die") and agree. If this ever
/// fails, the grade work has silently changed an answer for existing users.
#[test]
fn both_policies_agree_on_every_ungraded_fixture() {
    let mut ungraded = 0;
    for (name, text) in valid_fixtures() {
        let map = WaferMap::parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let uses_only_grade_1 = (0..BOARD_SIZE)
            .all(|r| (0..BOARD_SIZE).all(|c| map.get(r, c).grade().is_none_or(|g| g == Grade::G1)));
        if !uses_only_grade_1 {
            continue;
        }
        ungraded += 1;
        assert_eq!(
            find_best_region_with(&map, TieBreak::Grade),
            find_best_region_with(&map, TieBreak::Total),
            "{name}: policies must agree on an ungraded wafer"
        );
    }
    assert!(ungraded >= 10, "only {ungraded} ungraded fixtures found");
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
    assert_eq!(recovered.stats.good_total(), 57);

    let best = find_best_region(&map).unwrap();
    assert_eq!(best, recovered, "re-solving must find the same region");
    // Byte-identical below the fixture's leading note lines.
    let report = render_report(&map, &best, TieBreak::Grade);
    assert!(
        text.ends_with(&report),
        "re-rendering must reproduce the fixture body"
    );
}

/// A version-2 report, whose in-region good die are spelled `Z`, must still
/// parse and still reveal its region -- an old file stays valid input. It does
/// not round-trip byte for byte, because re-rendering upgrades `Z` to `A`; the
/// die states and the region are what must survive.
#[test]
fn legacy_v2_fixture_parses_and_upgrades_to_the_v3_alphabet() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../testdata/legacy_z_roundtrip.txt"
    ))
    .unwrap();
    assert!(text.contains('Z'), "fixture must exercise the v2 glyph");

    let map = WaferMap::parse(&text).expect("a v2 report must still parse");
    let recovered = map.marked_region().expect("region should be recoverable");
    assert_eq!((recovered.row, recovered.col), (2, 4));
    assert_eq!(recovered.stats.good_total(), 57);
    assert_eq!(
        recovered.stats.grade(Grade::G1),
        57,
        "v2 good die are grade 1"
    );

    // Its die states match the v3 fixture for the same wafer, so the two
    // spellings really do describe the same result.
    let v3 = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../testdata/marked_roundtrip.txt"
    ))
    .unwrap();
    let v3_map = WaferMap::parse(&v3).unwrap();
    for r in 0..BOARD_SIZE {
        for c in 0..BOARD_SIZE {
            assert_eq!(map.get(r, c), v3_map.get(r, c), "die state at ({r},{c})");
        }
    }
    assert_eq!(map.marked_region(), v3_map.marked_region());

    // Re-rendering emits the v3 alphabet, never `Z`.
    let report = render_report(&map, &recovered, TieBreak::Grade);
    assert!(!report.contains(LEGACY_IN_REGION_GOOD));
    assert!(v3.ends_with(&report), "must render as the v3 fixture body");
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
    let report = render_report(&map, &find_best_region(&map).unwrap(), TieBreak::Grade);
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

// ---------------------------------------------------------------------------
// The HTML report. The page is written to arbitrary locations and opened
// directly from disk, so the tests here are mostly about what must *not* be in
// it (external references, scripts, run-varying content) and about the grid
// agreeing with the text report it sits beside.
// ---------------------------------------------------------------------------

/// Solves `text` and renders both faces of the result.
fn html_for(text: &str, tie_break: TieBreak) -> (WaferMap, BestRegion, String) {
    let map = WaferMap::parse(text).expect("fixture must parse");
    let best = find_best_region_with(&map, tie_break).expect("fixture must have a legal region");
    let html = render_html(&map, &best, tie_break, Some("wafer.txt"));
    (map, best, html)
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// The `data-glyph` values in document order, i.e. the grid as the page draws
/// it (the glyph is rendered by CSS from this attribute).
fn glyphs_in(html: &str) -> String {
    html.split("data-glyph=\"")
        .skip(1)
        .map(|rest| {
            let value = unescape(rest.split('"').next().unwrap());
            assert_eq!(value.chars().count(), 1, "one glyph per cell: {value:?}");
            value
        })
        .collect()
}

/// The report embedded in the page's `<details>`, un-escaped.
fn embedded_report(html: &str) -> String {
    let body = html
        .split_once("<pre>")
        .expect("the page carries the raw report")
        .1
        .split_once("</pre>")
        .expect("unterminated <pre>")
        .0;
    unescape(body)
}

/// The page has to render from wherever it was written -- a sibling directory,
/// an attachment, a machine with no network -- so it may not reference anything
/// outside itself, and it must not need script to be readable.
#[test]
fn html_report_is_self_contained() {
    for (name, text) in [("ungraded", SAMPLE), ("graded", &graded_sample()[..])] {
        let (_, _, html) = html_for(text, TieBreak::Grade);
        assert!(html.starts_with("<!doctype html>\n"), "{name}");
        assert!(html.ends_with("</html>\n"), "{name}");
        for forbidden in ["<script", "href=", "src=", "http://", "https://", "@import"] {
            assert!(
                !html.contains(forbidden),
                "{name}: page must not contain {forbidden}"
            );
        }
        // A leftover reference to the web page's stylesheet variables would
        // render as an unstyled fallback here, since Pico is not loaded.
        assert!(!html.contains("var(--pico-"), "{name}");
    }
}

/// Same input, same bytes: no timestamp or other run-varying content, so two
/// reports can be diffed and a stale one is visible as a difference.
#[test]
fn html_report_is_deterministic() {
    let map = WaferMap::parse(&graded_sample()).unwrap();
    let best = find_best_region(&map).unwrap();
    let once = render_html(&map, &best, TieBreak::Grade, Some("wafer.txt"));
    let twice = render_html(&map, &best, TieBreak::Grade, Some("wafer.txt"));
    assert_eq!(once, twice);

    // The source label is optional, and its absence must not break the page.
    let anonymous = render_html(&map, &best, TieBreak::Grade, None);
    assert!(anonymous.starts_with("<!doctype html>\n"));
    assert!(!anonymous.contains("wafer.txt"));
}

/// One cell per die site, and exactly the mask's worth of them marked as being
/// in the region -- the tightest available check that the drawn region is the
/// one the solver chose.
#[test]
fn html_report_draws_every_cell_and_marks_only_the_region() {
    let (_, _, html) = html_for(&graded_sample(), TieBreak::Grade);
    assert_eq!(
        html.matches("class=\"wafer-cell\"").count(),
        BOARD_SIZE * BOARD_SIZE
    );
    assert_eq!(
        html.matches("data-region=\"true\"").count(),
        mask_site_count()
    );
    assert_eq!(
        html.matches("data-region=\"false\"").count(),
        BOARD_SIZE * BOARD_SIZE - mask_site_count()
    );
}

/// The picture and the text must be the same result: the glyph the page draws
/// in each cell is the character the `.txt` report writes there.
#[test]
fn html_report_grid_matches_the_marked_text() {
    for tie_break in TieBreak::ALL {
        let (map, best, html) = html_for(&graded_sample(), tie_break);
        let marked: String = mark_region(&map, &best).replace('\n', "");
        assert_eq!(glyphs_in(&html), marked, "{tie_break}");
    }
}

/// `data-grade` drives the colour ramp, so it must be present exactly on good
/// die and carry the right grade -- a cell coloured for the wrong grade would
/// misreport the very number being maximized.
#[test]
fn html_report_grade_attributes_match_the_glyphs() {
    let (map, best, html) = html_for(&graded_sample(), TieBreak::Grade);
    let marked = mark_region(&map, &best);

    let mut good = 0;
    for g in Grade::BEST_FIRST {
        let die = Die::Good(g);
        let expected = marked
            .chars()
            .filter(|&ch| ch == die.to_char(false) || ch == die.to_char(true))
            .count();
        assert!(expected > 0, "fixture should use grade {}", g.number());
        assert_eq!(
            html.matches(&format!("data-grade=\"{}\"", g.number()))
                .count(),
            expected,
            "grade {}",
            g.number()
        );
        good += expected;
    }
    // Nothing but a good die carries a grade. Counting the double-quoted form
    // keeps this to attributes: the stylesheet's selectors are single-quoted.
    assert_eq!(html.matches("data-grade=\"").count(), good);
    assert_eq!(html.matches("data-state=\"good\"").count(), good);
}

/// The page embeds the text report verbatim, so a report separated from its
/// `.txt` sibling can still be recovered from it -- including the `tiebreak=`
/// header that makes the result reproducible.
#[test]
fn html_report_embeds_a_reparseable_report() {
    for tie_break in TieBreak::ALL {
        let (map, best, html) = html_for(&graded_sample(), tie_break);
        let embedded = embedded_report(&html);
        assert_eq!(
            embedded,
            render_report(&map, &best, tie_break),
            "{tie_break}"
        );

        let reparsed = WaferMap::parse(&embedded).expect("embedded report must parse");
        assert_eq!(reparsed.marked_region(), Some(best), "{tie_break}");
        assert_eq!(
            reparsed.header_tie_break().and_then(|r| r.as_ref().ok()),
            Some(&tie_break)
        );
    }
}

#[test]
fn html_report_shows_the_headline_numbers() {
    let (_, best, html) = html_for(&graded_sample(), TieBreak::Grade);
    let s = best.stats;
    for expected in [
        format!("<strong>{}</strong> grade-4 die", s.grade(Grade::G4)),
        format!("<strong>{}</strong> good die in total", s.good_total()),
        format!("<strong>{}</strong> defect die", s.defect),
        format!("<strong>{}</strong> overhang site(s)", s.overhang),
        format!("<strong>{}</strong> die sites", s.sites()),
        format!("<strong>{:.1}%</strong> yield", s.yield_fraction() * 100.0),
        format!("row <strong>{}</strong>", best.row),
        format!("col <strong>{}</strong>", best.col),
    ] {
        assert!(html.contains(&expected), "missing {expected:?}");
    }
    // Every grade is broken out, best first, matching the legend's order.
    for g in Grade::BEST_FIRST {
        assert!(html.contains(&format!("<li>{} grade-{}</li>", s.grade(g), g.number())));
    }
    assert!(html.contains("ties broken by <strong>grade</strong>"));
}

/// The source label is a path, and a path can contain anything.
#[test]
fn html_report_escapes_its_source_label() {
    let map = WaferMap::parse(SAMPLE).unwrap();
    let best = find_best_region(&map).unwrap();
    let html = render_html(
        &map,
        &best,
        TieBreak::Grade,
        Some("<script>alert(1)</script> a&b \"q\""),
    );
    assert!(!html.contains("<script"));
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; a&amp;b &quot;q&quot;"));
}
