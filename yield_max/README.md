### Yield Max
This is a rust tool that analyzes a 300mm wafer map text file provided by the user as a text file input and locates the highest yielding 200mm region on the wafer. Good die are **graded** 1 through 4, and the region it picks is the one covering the most **grade-4** die.

The text file provided by the user contains an ASCII representation of the 300mm wafer with each die represented by a single character in a 17x17 grid. A sample of the file format is shown below:

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

The '.' character represents a non-present die.
The 'X' character represents a defect die.
The characters '1', '2', '3' and '4' each represent a good die, with the digit
giving its **grade** (bin or speed grade). All four are good die and all four
count toward the yield; grade 4 is the best.

A map that uses only '1' -- every wafer map written before grades existed -- is
still valid input and still gets the same answer. '1' is grade 1, a first-class
member of the grade set, not a deprecated spelling of "ungraded".

The 200mm wafer shape is an 11x11 grid. An example is shown below:
...OOOOO...
..OOOOOOO..
.OOOOOOOOO.
OOOOOOOOOOO
OOOOOOOOOOO
OOOOOOOOOOO
OOOOOOOOOOO
.OOOOOOOOO.
.OOOOOOOOO.
..OOOOOOO..
....OOO....

The '.' character represents a non-present die.
The 'O' character represents a present die in the 200mm wafer.

The goal of this program is to find the 200mm region of the 300mm wafer that maximizes the number of **grade-4** die from the 300mm wafer overlayed with the 'O' character from the 200mm mask. The 200mm region must land entirely within the 300mm wafer -- no part of the mask may hang off the edge onto a non-present ('.') die -- so a placement that would overhang is never a legal candidate, however many good die it covers. On top of that, the region must also be inset by at least one die from the wafer's true edge: a placement is not legal if it covers a die that itself sits on the boundary of the wafer's present-die area (off the grid entirely, or adjacent -- including diagonally -- to a non-present ('.') site), even though that die is present. The program should report the number of good die of each grade in the 200mm region and generate a new version of the 300mm text file that marks all die in the optimal 200mm region.

## The objective: grade 4 first

The figure being maximized is the count of grade-4 die, full stop. A placement
with one more grade-4 die wins even if it gives up a great many good die of
other grades, and even if its overall yield is worse.

What *isn't* fixed is how to settle a **tie** on that count, because the two
reasonable answers optimize different things:

| `--tiebreak=` | prefers | for someone who wants |
|---|---|---|
| `grade` (default) | the better remaining grades: most grade-3, then grade-2, then grade-1 | top-bin volume |
| `total` | the most good die overall, of any grade | total sellable die |

Placements that even these cannot separate fall back to the row-major-first
rule, as before.

Worked example -- `testdata/tiebreak_divergent.txt` has two placements tied at
17 grade-4 die:

```
$ yield_max --tiebreak=grade testdata/tiebreak_divergent.txt
Best 200mm region: top-left at (row 2, col 2)
  center die at H8 (row 7, col 7)
  17 grade-4 die (the figure being maximized)
  64 good die (17 grade-4, 16 grade-3, 10 grade-2, 21 grade-1), ...

$ yield_max --tiebreak=total testdata/tiebreak_divergent.txt
Best 200mm region: top-left at (row 4, col 2)
  center die at J8 (row 9, col 7)
  17 grade-4 die (the figure being maximized)
  68 good die (17 grade-4, 14 grade-3, 15 grade-2, 22 grade-1), ...
```

Neither is wrong: `grade` trades 4 good die for 2 more grade-3 die. Note that
the grade-4 count is 17 either way -- the policy never touches the objective.

A **weighted score** (say `8*n4 + 4*n3 + 2*n2 + n1`) was rejected: it lets 8
grade-3 die outrank a single grade-4 die, which contradicts the requirement,
and it invents magic numbers with no physical meaning.

On a wafer that uses only grade 1, both policies reduce to "most good die" --
the original objective -- so no pre-existing wafer map changes its answer. The
test suite asserts this directly (`both_policies_agree_on_every_ungraded_fixture`)
and every fixture predating grades still carries its original expectation.

## Reading the grid: row letters and column numbers

Version 4 gives every die site a name, because "row 7, col 9" is easy to lose
your place in when you are counting characters in a 17x17 block of ASCII art,
and because a result that names a site can be checked against the picture
without counting at all.

- **Columns** are numbered `1`..`17`, left to right.
- **Rows** are lettered top to bottom, starting at `A`, **skipping `N`**:
  `A B C D E F G H I J K L M O P Q R`. `N` is left out on purpose: beside a
  wafer map full of absent sites it reads as "no"/"none", and `M`/`N` are the
  easiest pair to confuse when a column of letters is read aloud.
- A site is named **`<letter><number>`** with no separator: `A1` is the
  top-left site, `R17` the bottom-right. Letters and digits cannot collide, so
  nothing is needed between them.

The labels are **additive**. The 0-based `row=`/`col=` numbers stay exactly
where they were — in the report header, in JSON, in the wasm API — so nothing
that does arithmetic on a position has to learn the notation, and a report
still round-trips. Error messages keep their existing 1-based row/col rule.

`ROW_LABELS` in `yield_max-core` is the single source of truth for the skip;
the web UI and the HTML report both read it back rather than regenerating it.

### The center die

The result names the **center die** of the winning region: the site at the
middle of the 11x11 mask, offset (5, 5) from its top-left corner. The mask's
middle site is present (`O`), so the center is always a real, addressable die
rather than a point between them — which is why it is reported instead of the
region's centroid, a quantity the mask's up-down asymmetry puts off-lattice
anyway.

For the sample answer (row 2, col 4) the center die is grid (7, 9), i.e.
**H10**. It appears in the report header (`center=H10`), in the CLI summary,
in JSON, in the HTML report and in the web UI, where it is also ringed on the
grid so it can be found by looking.

## Output format (version 4)

The original spec marked every die in the winning region with a single `Z`.
That answered "where" but not "why": a region of 93 sites that captures 57 good
die is also carrying 36 defect die, and none of that survived into the output
file.

Each cell carries two orthogonal facts — the **die state** (including a good
die's grade) and whether the cell is **inside the region** — so the output
alphabet gives every combination a distinct glyph:

|          | outside region | inside region |
|----------|----------------|---------------|
| grade 4  | `4`            | `D`           |
| grade 3  | `3`            | `C`           |
| grade 2  | `2`            | `B`           |
| grade 1  | `1`            | `A`           |
| defect   | `X`            | `*`           |
| absent   | `.`            | `-`           |

Grade order maps to alphabet order (`A`..`D` for grades 1..4), so an in-region
glyph is readable without consulting the legend, and the two spellings of a
grade sort together in the same relative order.

Version 2 had a single in-region good glyph, `Z`. It is **still accepted on
input** — read as an in-region grade-1 die — so a report from an older run
remains valid input, but it is never emitted: version 3 onward writes `A`.
Feeding a version-2 report back in therefore upgrades it to the current
alphabet (and gains axis labels) rather
than reproducing it verbatim; see `testdata/legacy_z_roundtrip.txt`.

`-` (overhang) marks a region cell that falls outside the wafer's present-die
area. A 200mm region is only legal if *none* of its sites land there (see
"Ties and edge cases" below), so `-` never appears in a report the tool
computes itself — but the glyph stays part of the alphabet, since it's still
needed to round-trip a file marked by an older run or hand-edited to explore
an illegal placement.

Case pairs (`Z`/`z`, `D`/`d`) were rejected deliberately: in 17x17 monospace
ASCII art case is nearly invisible, and it makes every downstream consumer
fragile to case-folding. `#` is reserved as the comment sigil and so is not
used for a die state.

Lines beginning with `#` are comments, ignored on input. Output carries a
five-line header so the file is self-describing and greppable — the headline
numbers, the legend, and the column numbers as two lines (tens over units,
because 17 columns cannot be numbered on one line of single characters). Each
grid row is prefixed with its row letter and a space, which is exactly the
width of the `# ` opening the header lines, so column *k* sits at the same
offset in the header and in every row:

```
# yield_max 4  region=row3,col4 center=I10 tiebreak=grade
# good=70 (g4=21 g3=18 g2=11 g1=20) defect=23 overhang=0 sites=93 yield=75.3%
# in-region: D=good4 C=good3 B=good2 A=good1 *=defect -=overhang   outside: 4/3/2/1=good X=defect .=absent
#          11111111
# 12345678901234567
A .....23X2214.....
B ...3X33114X343...
C ..X331X4113XX13..
D .3X43X1D*D*C1413.
E .X3433*CCDB*B3XX.
F 12X13AB*AAB**D212
G 432XAD*DD*BBAAC14
H X23XDD*AA*CD*CD4X
I 1X3XA*A*AACCADD34
J 1343DAAACB***C*X2
K X2142CDD*DAC*B3X1
L .1X42*BACACB**X2.
M .32XX3BDDCCDC33X.
O ..323324CDAX333..
P ...2X121X3X414...
Q ....32133142X....
R .......3124......
```

`good` is every grade summed — the same quantity version 2 called `good` — with
the per-grade breakdown in parentheses beside it. `yield` is good die over
*present* die (good + defect); the denominator excludes overhang, though a
legal region now always has zero of it, so in practice `yield` is just good
divided by the 93 mask sites.

`tiebreak` records which policy produced this result, because the result is not
reproducible without it. The tool reads it back: re-running on a report with no
`--tiebreak` flag reuses the recorded policy, so **the output is valid input**
under either policy — re-running the tool on its own report reproduces it byte
for byte, and the recorded region can be recovered with
`WaferMap::marked_region()`. A `--tiebreak` that *contradicts* the header is an
error rather than a silent re-solve, since the result would be
indistinguishable on sight from the report it replaced.

## Input validation

Anything that is not unambiguously a wafer map is rejected; nothing malformed
is silently reinterpreted. Inputs over 64 KB are refused (checked against the
file size before reading, so a huge file costs a stat rather than the memory).

Two cases get special handling because the naive behavior misleads:

- A **UTF-8 BOM** is stripped. Left in place it reports "row 1 has length 18"
  about a row the user can see is 17 characters, with an invisible cause.
- **Invisible or lookalike characters** are named in the error, not printed:
  a tab, a non-breaking space, or a fullwidth `１` would otherwise produce a
  message that looks blank or identical to a legal one.

**Free-text header lines** above the grid -- lot number, slot, operator,
timestamp, anything with no `#` marker -- are tolerated too: any leading line
that is neither exactly 17 characters wide nor a labeled grid row (a row
letter, a space, then 17 characters) is dropped before the grid is parsed.
A leading line that does happen to be 17 characters wide is left alone and
runs through normal validation instead, since at that width it can't be told
apart from a genuinely malformed first grid row.

Marks in the input that match no legal mask placement are overwritten by the
run's result, but the tool **warns first** rather than discarding a hand edit
silently.

## Ties and edge cases

- **Ties** on the grade-4 count are settled by `--tiebreak` (see "The
  objective" above); placements that policy cannot separate either are broken
  in favor of the earlier placement in row-major order.
- A 200mm region must land **entirely on present die**: any placement where
  the mask would hang off the wafer edge onto an absent (`.`) site is not
  legal and is never considered, however many good die it covers.
- A 200mm region must also keep **at least one die of clearance from the
  wafer's true edge** on every side: a placement is illegal if any of its
  sites is itself a die that sits on the boundary of the wafer's present-die
  area — off the 17x17 grid entirely, or adjacent (including diagonally) to
  an absent (`.`) site — even when that die is present. This rules out row 0,
  row 16, column 0, and column 16 outright (a die there always borders the
  grid edge), and can rule out interior placements too, near the wafer's
  rounded corners.
- If *no* placement anywhere satisfies both constraints — the wafer's
  present-die area, once inset by one die on every side, is smaller than the
  mask everywhere it could sit — there is no legal region at all, and the
  tool reports an error instead of a result.
- A **blank line inside the grid** is an error, not something to silently skip
  — dropping it would shift every later row and yield a confidently wrong
  answer. Blank lines around the grid, CRLF endings, and trailing spaces are
  all tolerated.
- A **row label that disagrees with its position** is an error for the same
  reason: it means a row was inserted, dropped or reordered, and stripping the
  label to trust the position would answer confidently about a different
  wafer. So is a **partially labeled** grid — labeling is all-or-nothing per
  file. An unlabeled 17-wide grid is not an error at all: every map written
  before version 4 is one, and it means exactly what it always did.
- Row and column numbers in **error messages** are 1-based, matching a text
  editor. Row/col in the output header and JSON are 0-based grid indices; the
  `A1`-style labels are carried alongside them, never instead.

## Usage

```
usage: yield_max [options] <input_path> [output_path]

  --tiebreak=P   Settle a tie on the grade-4 count: 'grade' (default) or
                 'total'. See "The objective" above.
  --json         Machine-readable JSON on stdout.
  -h, --help     Show help.
```

Output defaults to `<input>_optimal.txt` beside the input; `-` writes the
report to stdout. The tool refuses to overwrite its own input file.

### The HTML report

Every run also writes an **HTML report** of the same result: the marked wafer
as a colour-coded grid with the chosen region outlined, the headline numbers,
the legend, and the text report itself in a collapsed `<details>`. It is the
same view the web frontend shows, so a CLI run can be opened in a browser or
attached to a ticket without pasting anything into the page.

It goes beside the text report with a `.html` extension —
`<input>_optimal.html` by default, `<output>.html` when an output path is
given, and `<input>_optimal.html` when the report goes to stdout, since there
is no output path to sit beside. Asking for the *text* report at a `.html`
path is an error rather than a silent clobber.

Two properties the renderer holds to, both tested:

- **Self-contained** — styles are inline and there is no script and no external
  reference of any kind, so the file renders from wherever it was written, with
  no network.
- **Deterministic** — no timestamp or other run-varying content, so two reports
  of the same input are byte-identical and diffable.

`--json` is the interface for other programs — don't parse the ASCII art:

```json
{"version":4,"tiebreak":"grade",
 "best":{"row":2,"col":4,"label":"C5",
         "center":{"row":7,"col":9,"label":"H10"},
         "good":57,
         "good_by_grade":{"4":0,"3":0,"2":0,"1":57},
         "defect":36,"overhang":0,"sites":93,"yield":0.6129},
 "mask_sites":93,"output":"wafer_optimal.txt",
 "html":"wafer_optimal.html"}
```

`good` keeps its version-2 meaning (all grades summed), so a consumer reading
it is not silently handed a subset; `good_by_grade` is additive alongside it.
`label` and `center` are likewise additive — `row`/`col` are unchanged in
shape and meaning, so nothing has to adopt the notation. `version` bumping to
4 is the tripwire for anything that needs to care; it moved because the grid
*text* changed shape (labels), and it is shared with the text report's
`# yield_max 4` header.

If no placement anywhere satisfies both the overhang and edge-clearance
constraints, the tool exits with an error instead (nonzero exit code, message
on stderr) rather than emitting JSON or a report. An unrecognized
`--tiebreak` value is likewise an error naming the valid ones, not a silent
fallback to the default.

## Layout

- `yield_max-core/` — dependency-free solver and file format. Single source of
  truth for the mask shape (`MASK_TEMPLATE`), the cell alphabet (`Die`,
  `Grade`), the axis labels (`ROW_LABELS`, `cell_name`), and the tie-break
  policies (`TieBreak`). Both output faces live
  here too: `render_report` (text) and `render_html`, so the numbers, glyphs
  and legend cannot differ between them.
- `yield_max-cli/` — command-line frontend.
- `yield_max-wasm/` — `wasm-bindgen` wrapper; also re-exports the mask via
  `mask_rows()`, the grades via `grades_best_first()`, the policies via
  `tie_breaks()` and the axis labels via `row_labels()`/`col_labels()`, so the
  web UI cannot drift out of sync with the solver.
- `index.html` — web frontend (file upload, visualization, report download).

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd yield_max-wasm && wasm-pack build --target web   # pkg/ is checked in
```
