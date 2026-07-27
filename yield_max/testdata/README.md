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
| `all_good` | 0,0 | 93 | 0 | 0 | All 49 placements tie; the row-major first-wins tie-break must pick (0,0). |
| `all_defect` | 0,0 | 0 | 93 | 0 | Tie-break again, at zero good die, with a non-zero yield denominator. |
| `all_absent` | 0,0 | 0 | 0 | 93 | Divide-by-zero guard: no present die at all. |
| `single_good_die` | 6,2 | 1 | 92 | 0 | One good die near the bottom edge forces the winner away from (0,0). |
| `center_cluster` | 3,3 | 69 | 24 | 0 | Region settles centrally rather than drifting to an edge. |
| `corner_nw` | 0,0 | 53 | 34 | 6 | Minimum offset (0,0) reached for a real reason, not the tie-break. |
| `corner_se` | 6,6 | 75 | 9 | 9 | **Maximum offset (6,6)** — the far end of the placement range. |
| `corner_overhang` | 0,6 | 50 | 37 | 6 | Region pinned to a corner where overhang, not good count, is the story. |
| `edge_ring_defects` | 3,3 | 93 | 0 | 0 | Classic edge-exclusion ring; a perfect region exists inside it. |
| `messy_whitespace` | 0,5 | 63 | 29 | 1 | CRLF, trailing spaces, and surrounding blank lines are all tolerated. |
| `header_text` | 0,5 | 63 | 29 | 1 | Free-text metadata lines above the grid, with no `#` marker, are tolerated. |
| `marked_roundtrip` | 0,5 | 63 | 29 | 1 | The tool's own output; re-running must reproduce it byte for byte. |
| `utf8_bom` | 0,5 | 63 | 29 | 1 | A BOM at byte 0 is stripped, not reported as a bogus 18-character row. |

Between them `corner_nw`, `corner_se`, and `corner_overhang` cover both ends of
the row and column placement range, which is where off-by-one errors surface.

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
last-wins, counting overhang as good, or skipping the maximum offset each cause
failures here.
