# Test fixtures

Wafer maps used by `yield_max-core`'s test suite. Each valid fixture carries
its own answer in a `# expect:` header, so the assertion lives next to the data
and adding a fixture needs no test-code change — drop the file in and
`every_valid_fixture_parses_and_matches_its_expected_result` picks it up.

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

## Regenerating

`all_good`, `corner_se`, and the rest are generated from predicates rather than
hand-typed. The tests are mutation-checked: flipping the tie-break to
last-wins, letting an overhang placement win, letting a placement touch the
wafer's edge, or skipping the maximum legal offset each cause failures here.
