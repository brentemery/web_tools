# Test fixtures

Wafer maps used by `yield_max-core`'s test suite. Each valid fixture carries
its own answer in a `# expect:` header, so the assertion lives next to the data
and adding a fixture needs no test-code change — drop the file in and
`every_valid_fixture_parses_and_matches_its_expected_result` picks it up.

A fixture whose answer differs under the two tie-break policies carries a
second `# expect-total:` line for `TieBreak::Total`; the `# expect:` line is
always the default (`TieBreak::Grade`). Per-grade counts are written `g4=`..
`g1=`. A fixture that gives only `good=` is taken to be all grade 1, which is
why the fixtures predating graded die needed no edit — and their answers being
unchanged is itself the backward-compatibility evidence.

**The expected values were computed by an independent Python implementation of
the search, not recorded from this code.** That distinction matters: fixtures
generated from the tool's own output can only pin current behavior, whereas
these can catch a genuine regression.

| fixture | row,col | good | defect | overhang | what it pins down |
|---|---|---|---|---|---|
| `all_good` | 1,1 | 93 | 0 | 0 | All 25 legal placements tie; the row-major first-wins tie-break must pick (1,1) — (0,0) is never legal. |
| `all_defect` | 1,1 | 0 | 93 | 0 | Tie-break again, at zero good die, with a non-zero yield denominator. |
| `all_absent` | — | — | — | — | No die sites anywhere, so every placement is entirely overhang: `find_best_region` must return `None`. |
| `single_good_die` | 5,2 | 1 | 92 | 0 | One good die at row 15 (row 16 can never be covered) forces the winner away from (1,1). |
| `center_cluster` | 3,3 | 69 | 24 | 0 | Region settles centrally rather than drifting to an edge. |
| `corner_nw` | 1,1 | 44 | 49 | 0 | Minimum *legal* offset (1,1) reached for a real reason, not the tie-break — (0,0) itself is always illegal. |
| `corner_se` | 5,5 | 70 | 23 | 0 | **Maximum legal offset (5,5)** — (6,6) itself is always illegal, so this is the true far end of the placement range. |
| `corner_overhang` | 1,3 | 30 | 63 | 0 | Two higher-good placements are rejected for two different reasons: (0,6) has 6 sites of overhang; (0,4) has zero overhang but still sits on the wafer's edge (row 0). |
| `edge_ring_defects` | 3,3 | 93 | 0 | 0 | Classic edge-exclusion ring; a perfect region exists inside it. |
| `messy_whitespace` | 2,4 | 57 | 36 | 0 | CRLF, trailing spaces, and surrounding blank lines are all tolerated. |
| `header_text` | 2,4 | 57 | 36 | 0 | Free-text metadata lines above the grid, with no `#` marker, are tolerated. |
| `marked_roundtrip` | 2,4 | 57 | 36 | 0 | The tool's own output; re-running must reproduce it byte for byte. |
| `utf8_bom` | 2,4 | 57 | 36 | 0 | A BOM at byte 0 is stripped, not reported as a bogus 18-character row. |
| `legacy_z_roundtrip` | 2,4 | 57 | 36 | 0 | A **version-2** report, whose in-region good die are spelled `Z`. Must still parse (as in-region grade-1 die) so an old file stays valid input; re-rendering upgrades it to `A`, so this one deliberately does *not* round-trip byte for byte. |

## Graded fixtures

These exercise good-die grades and the grade-4 objective. `row,col` is the
placement under the default `grade` policy.

| fixture | row,col | g4 | g3 | g2 | g1 | what it pins down |
|---|---|---|---|---|---|---|
| `grades_mixed` | 3,4 | 21 | 18 | 11 | 20 | **The fixture that proves the objective changed.** The winner carries 70 good die, while (3,2) carries 72 — but only 19 grade-4 against the winner's 21. Maximizing grade 4 must beat maximizing good die. |
| `grade4_cluster` | 4,4 | 24 | 0 | 0 | 23 | The same trade with a wide margin: a grade-4 clump ringed by defects (47 good die) beats a clean grade-1 field at (1,3) (62 good die, only 8 grade-4). Giving up 15 good die for 16 grade-4 keeps the fixture discriminating even if the mask or legality rules are retuned. |
| `grade4_tie` | 3,4 | 6 | 27 | 13 | 25 | Three placements tie at the maximum grade-4 count (6); the grade ladder settles it on grade-3 count — (3,4) has 27 against 26 at (3,3) and 24 at (4,2). The winner is *not* the row-major-first tied placement, so the positional rule cannot explain the answer. As a bonus, (2,4) carries the most good die (75) and the most grade-3 (30) yet loses on grade 4, so the fixture also pins the grade-4 lead. |
| `grade4_all_tie` | 1,3 | 93 | 0 | 0 | 0 | Every present die is grade 4, so all 12 legal placements have identical histograms and neither policy can separate them. Only the row-major-first rule is left. |
| `tiebreak_divergent` | 2,2 | 17 | 16 | 10 | 21 | **The fixture that justifies `--tiebreak`.** Both policies agree on 17 grade-4 die, then diverge: `grade` picks (2,2) (16 grade-3, 64 good), `total` picks (4,2) (14 grade-3, 68 good). Carries an expectation for each. |
| `graded_roundtrip` | 3,4 | 21 | 18 | 11 | 20 | The tool's own version-3 output, which must reproduce byte for byte on re-run. Exercises all four in-region good glyphs (`A`/`B`/`C`/`D`) on the parse side. |

Every fixture above has zero overhang: a placement is not a legal 200mm
region unless it lands entirely on present die, *and* keeps at least one die
of clearance from the wafer's true edge on every side. Row 0, row 16, column
0, and column 16 are consequently never legal for any wafer, which is why
every fixture above resolves to a placement no wider than the 1..=5 range —
see `corner_nw` and `corner_se`, which pin down that far range on the
row-major-first side and the maximum side respectively, and
`corner_overhang`, which pins down that the edge rule rejects a placement
even when it has zero overhang. `all_absent` pins down the case where no
legal placement exists anywhere on the wafer.

## invalid/

Files that must be **rejected**, each naming its expected error. Covers wrong
row count and row length, characters outside the alphabet, a lowercase `z`
(case pairs were deliberately rejected as a format choice, so `z` is not an
alias for `Z`), an empty file, and CR-only line endings.

Three cover characters that are invisible or misleading on screen, where the
error message has to name what it found rather than print it: `tab_character`,
`nbsp_character` (U+00A0, indistinguishable from a space), and `fullwidth_one`
(U+FF11, a homoglyph that reads as a good die but is not one).

`comment_inside_grid.txt` mirrors `blank_line_inside.txt`: comments are a
header/footer convention, so one interleaved with the grid means the file was
assembled wrongly.

`blank_line_inside.txt` is the important one: a blank line in the middle of the
grid used to be silently dropped, which shifted every later row up and produced
a confidently wrong answer with no error.

`stray_mark.txt` is the exception — it parses cleanly, but its marks match no
legal mask placement, so `marked_region()` must return `None` rather than
inventing a region.

Note `lowercase_z.txt` still applies with grades: case pairs remain rejected,
so neither `z` nor a lowercase `a`–`d` is an alias for its uppercase glyph.

## Regenerating

`all_good`, `corner_se`, and the rest are generated from predicates rather than
hand-typed. The graded fixtures go further: `grades_mixed`, `grade4_tie`, and
`tiebreak_divergent` were found by randomised search under a predicate
asserting the fixture actually *discriminates* — that the grade-4 winner is not
also the max-good-die placement, that the tied placements really are separated
by grade 3 and not by position, that the two policies really do pick different
regions. A fixture that passes for an unrelated reason is worse than none.

The tests are mutation-checked: flipping the tie-break to last-wins, letting an
overhang placement win, letting a placement touch the wafer's edge, skipping the
maximum legal offset, scoring all good die equally instead of leading with
grade 4, or swapping the two tie-break policies each cause failures here.
