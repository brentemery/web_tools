# Plan — axis labels and the region's center die (output format version 4)

Status: **proposed**, not yet implemented. Written against commit `9cb8cb9`.

## What is being asked

1. The output grid gets **column and row headers**: columns numbered from `1`
   at the left, rows lettered from `A` at the top, **skipping `N`** (so the
   17 rows are `A B C D E F G H I J K L M O P Q R`). `N` is skipped because in
   a fab context it reads as "no"/"none" beside a wafer map full of absent
   sites, and `N`/`M` are the easiest pair to confuse when reading a column of
   letters aloud.
2. For the winning placement, report the **location of the center die** of the
   region as (row, column) in that same notation.

The center of an 11x11 mask is the site at mask offset (5, 5), which is an `O`
in `MASK_TEMPLATE` — so the center is always a real die site, never overhang.
For a region with its top-left at grid (r, c) the center die is at
(r + 5, c + 5); at the sample answer (row 2, col 4) that is grid (7, 9), which
in the new notation is **H10**.

## Notation, precisely

| grid index (0-based) | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| row label | A | B | C | D | E | F | G | H | I | J | K | L | M | O | P | Q | R |
| column label | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 |

A cell is named **`<row letter><column number>`**, letter first, no separator:
`A1` is the top-left site, `R17` the bottom-right, `H10` the sample's center
die. Letter-first with no separator is the plate/well convention this reads
like, and it is unambiguous because the letter set and the digit set are
disjoint.

The existing 0-based `row=`/`col=` numbers **stay** everywhere they already
appear (report header, JSON, wasm getters, error messages keep their current
1-based-for-humans rule). Labels are added *beside* them, never as a
replacement: JSON consumers and the `marked_region()` round-trip must not have
to learn a new coordinate system, and a label is not arithmetic-friendly.

## Text report: version 4

```
# yield_max 4  region=row2,col4 center=H10 tiebreak=grade
# good=57 (g4=0 g3=0 g2=0 g1=57) defect=36 overhang=0 sites=93 yield=61.3%
# in-region: D=good4 C=good3 B=good2 A=good1 *=defect -=overhang   outside: 4/3/2/1=good X=defect .=absent
#          11111111
# 12345678901234567
A .....1111111.....
B ...XX111111XXX...
C ..X11X111X1X111..
...
R .......XXXX......
```

Decisions baked into that layout:

- **Column headers are `#` comment lines.** Two of them, tens over units,
  because columns run past 9 and a single line cannot number 17 columns of
  one character each. Being comments, they are already discarded by the
  existing parser's leading-comment rule — no new parse path is needed for
  them, and any older consumer that greps the grid out of a report keeps
  working.
- **Row labels are a two-character prefix** (`A` + one space), which is
  exactly the width of the `# ` that aligns the column headers above them. A
  grid line is therefore 19 characters, and column *k* sits at the same string
  offset in the header lines and in every grid row.
- **Labels are emitted, and accepted on input.** A report is still valid input
  to the tool (`README.md`'s round-trip property), so the parser has to read
  its own labels back. An **unlabeled** 17-wide grid — every wafer map written
  before this change, and anything hand-typed — stays valid and unchanged in
  meaning.
- **The version goes to 4** because the shape of the grid text changed. That
  is the tripwire the `version` field exists for, and it moves in the text
  header, in JSON, and in `# yield_max N` together, as version 3 did.

### Parsing labeled input

New rules, applied *before* the existing "drop leading lines that aren't 17
wide" header-text heuristic (which would otherwise eat all 17 labeled rows and
report `WrongRowCount(0)`):

- A line is a **labeled grid row** if its first character is a legal row letter
  and its second character is a space. No genuine grid row can look like this:
  space is not in the cell alphabet, so position 1 of an unlabeled row is
  always a cell glyph. (`A`..`D` being both row letters *and* in-region good
  glyphs is exactly why the space, not the letter, is the discriminator.)
- Labeling is **all-or-nothing per file**. If the first grid row is labeled,
  all 17 must be; a mix is `ParseError::MixedRowLabels`.
- A label that is present but **wrong for its position** is
  `ParseError::BadRowLabel { row, found, expected }`, not something to strip
  and forget. A file whose labels read `...L M N O...` has had a row inserted,
  deleted or reordered, and silently trusting position over label would give a
  confidently wrong answer — the same reasoning that already makes a blank
  line inside the grid an error.
- Column header comment lines are *not* validated. They are comments; they
  carry no information the row content doesn't, and demanding an exact match
  would reject a hand-annotated file for no gain.

New fixtures: `testdata/labeled_roundtrip.txt` (a v4 report, round-trips byte
for byte), and under `testdata/invalid/`: `mislabeled_row.txt` (`N` used),
`skipped_row_label.txt` (a row dropped, so labels jump), `mixed_labels.txt`
(some rows labeled). `testdata/README.md` gains rows for each.

## The center die, everywhere the region is reported

| surface | before | after |
|---|---|---|
| text header | `region=row2,col4` | `region=row2,col4 center=H10` |
| CLI summary | `Best 200mm region: top-left at (row 2, col 4)` | adds `  center die at H10 (row 7, col 9)` |
| JSON | `"best":{"row":2,"col":4,...}` | adds `"center":{"row":7,"col":9,"label":"H10"}` and `"label":"C5"` for the top-left |
| HTML report | `top-left at (row 2, col 4)` | adds the center die, labeled |
| web UI stats | same wording as HTML | same |
| wasm `Placement` | `row`, `col` | adds `center_row`, `center_col`, `center_label`, `label` |

JSON keeps `row`/`col` where they are and shape-unchanged; `center` and
`label` are additive, but they ride the version-4 bump anyway since the text
format forces one.

## Core API (single source of truth, as with `MASK_TEMPLATE`)

In `yield_max-core`:

```rust
/// Row letters, top to bottom. 'N' is skipped deliberately (see README).
pub const ROW_LABELS: [char; BOARD_SIZE] = ['A','B',...,'M','O',...,'R'];

pub fn row_label(row: usize) -> char;          // 7 -> 'H'
pub fn row_index(label: char) -> Option<usize>; // 'H' -> 7, 'N' -> None
pub fn col_label(col: usize) -> usize;          // 9 -> 10
pub fn cell_name(row: usize, col: usize) -> String; // (7, 9) -> "H10"

/// Offset of the mask's center site, and the grid position it lands on.
pub const MASK_CENTER: usize = MASK_SIZE / 2;
impl BestRegion {
    pub fn center(&self) -> (usize, usize);
    pub fn center_name(&self) -> String;
    pub fn name(&self) -> String;   // label of the top-left corner
}
```

The wasm crate re-exports `row_labels()` and `col_labels()` the way it already
re-exports `mask_rows()` and `grades_best_first()`, so `index.html` draws its
axes from the solver's list rather than generating a second copy of the
skip-`N` rule. A test asserts `ROW_LABELS` has no `N`, no duplicates, is
ascending, and that `row_index ∘ row_label` is the identity.

## HTML and web UI

Both grids become **18x18**: a header row of column numbers above, a header
column of row letters at the left, and an empty corner cell.

- `grid-template-columns: repeat(17, 1fr)` → `repeat(18, 1fr)` in *both*
  `html.rs`'s `STYLE` and `index.html` (they are deliberate copies; the plan
  keeps them character-identical in the parts that overlap).
- New `.wafer-axis` rule: no background, no border, `--text-secondary`,
  monospace, centered — visually a label, not a die, so a reader never
  mistakes the axis for a site.
- Accessibility: the axis cells become `role="columnheader"` /
  `role="rowheader"` inside the existing `role="grid"`, which is what those
  roles are for; the per-cell `aria-label` gains the name, reading
  `"H10 (row 7, col 9): Grade-4 good die — in selected region"`. Cell labels
  are generated in `html.rs` and mirrored in `index.html`'s `renderGrid`.
- The center die gets a small marker in both views — a dot drawn with the
  region accent via `.wafer-cell[data-center='true']::before` — so "where is
  the center" is answerable by looking, not only by reading the number. It is
  a separate element from the `::after` glyph, so no cell loses its glyph.

The HTML report's two invariants (self-contained, deterministic) are unaffected
and the existing tests keep guarding them.

## Test and CI changes

- `yield_max-core/src/tests.rs`:
  - `row_labels_skip_n_and_round_trip`
  - `cell_name_matches_the_documented_corners` (`A1`, `R17`, sample center
    `H10`)
  - `center_die_is_a_mask_site_and_lands_mid_region`
  - `report_header_records_the_center`
  - `labeled_output_round_trips_through_the_parser` (v4 report → same bytes)
  - `unlabeled_v3_input_still_parses_unchanged` (every pre-existing fixture
    already covers this, but assert it by name)
  - `rejects_mislabeled_and_mixed_label_rows`
  - HTML: `html_report_draws_both_axes`, and update the cell-count assertions
    from 289 to 289 dice + 35 axis cells.
- `yield_max-cli`: `json_report_shape` gains `center`.
- `smoke-test.mjs`: assert `center_label === 'H10'` for `test_wafer.txt`.
- `.github/workflows/ci.yml`: the end-to-end step's grep for the header line
  gains `center=H10`; the round-trip `diff` already covers labeled input once
  the report emits labels.

## Documentation

`README.md` gets a short **"Reading the grid"** section defining the labels
(including why `N` is skipped) and the center-die report, the version-3
format section becomes version 4 with the new sample, and the JSON sample
gains `center`. `CLAUDE.md`'s summary bullets follow. `testdata/README.md`
gains the three new fixtures.

## Rejected alternatives

- **Labels instead of `row=`/`col=` numbers.** Breaks every JSON consumer and
  makes the header unparseable back into a placement without a lookup table,
  for a readability gain that is fully served by carrying both.
- **Row letters `A`..`Q` with no skip.** Simpler, but the request is explicit,
  and a skipped letter costs nothing once `ROW_LABELS` is the single source.
- **`H-10` or `(H, 10)` as the cell name.** More typing, no less ambiguity —
  letters and digits already can't collide.
- **Column headers as plain (non-comment) text.** Would need a new parser
  branch and a new class of "is this a header or a bad grid row" ambiguity;
  `#` already means exactly "ignore this".
- **Spacing the grid out to `A  . . . 1 1` for wide column numbers.** Doubles
  the grid's width and destroys the ASCII-art readability that is the point
  of the format.
- **Reporting the region's centroid instead of its center site.** The mask is
  not symmetric top-to-bottom, so the centroid is not a lattice point; the
  center *die* is a real, addressable site and is what a fab asks for.

## Order of work

1. Core: labels, `BestRegion::center*`, header emission, label parsing +
   errors, fixtures, tests. (Everything else reads from here.)
2. CLI: summary line, JSON `center`, its tests.
3. wasm: getters + `row_labels()`/`col_labels()`; rebuild `pkg/`;
   `smoke-test.mjs`.
4. `index.html`: axes, center marker, labels in tooltips/stats.
5. `html.rs`: the same, kept in step with `index.html`.
6. README / CLAUDE.md / testdata README / CI.

Steps 1–2 are one commit, 3–5 one each, 6 folded into the commit that makes
each claim true.
