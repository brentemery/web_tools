### Yield Max
This is a rust tool that analyzes a 300mm wafer map text file provided by the user as a text file input and locates the highest yielding 200mm region on the wafer.

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
The '1' character represents a good die.

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

The goal of this program is to find the 200mm region of the 300mm wafer to maximize the number of good die in from the 300mm wafer overlayed with the 'O' character from the 200mm mask. The 200mm region must land entirely within the 300mm wafer -- no part of the mask may hang off the edge onto a non-present ('.') die -- so a placement that would overhang is never a legal candidate, however many good die it covers. The program should report the number of good die in the 200mm region and generate a new version of the 300mm text file that marks all die in the optimal 200mm region.

## Output format (version 2)

The original spec marked every die in the winning region with a single `Z`.
That answered "where" but not "why": a region of 93 sites that captures 62 good
die is also carrying 31 defect die, and none of that survived into the output
file.

Each cell carries two orthogonal facts — the **die state** and whether the cell
is **inside the region** — so the output alphabet gives all six combinations a
distinct glyph:

|        | outside region | inside region |
|--------|----------------|---------------|
| good   | `1`            | `Z`           |
| defect | `X`            | `*`           |
| absent | `.`            | `-`           |

`-` (overhang) marks a region cell that falls outside the wafer's present-die
area. A 200mm region is only legal if *none* of its sites land there (see
"Ties and edge cases" below), so `-` never appears in a report the tool
computes itself — but the glyph stays part of the alphabet, since it's still
needed to round-trip a file marked by an older run or hand-edited to explore
an illegal placement.

Case pairs (`Z`/`z`) were rejected deliberately: in 17x17 monospace ASCII art
case is nearly invisible, and it makes every downstream consumer fragile to
case-folding. `#` is reserved as the comment sigil and so is not used for a die
state.

Lines beginning with `#` are comments, ignored on input. Output carries a
three-line header so the file is self-describing and greppable:

```
# yield_max 2  region=row0,col4
# good=62 defect=31 overhang=0 sites=93 yield=66.7%
# in-region: Z=good *=defect -=overhang   outside: 1=good X=defect .=absent
.....11ZZZZZ.....
...XX1ZZZZZ**X...
..X11*ZZZ*Z*ZZ1..
.XXXZZZZZZZ*ZZZX.
.XXX*ZZZZ**ZZ*ZX.
XXXX*Z*ZZZZZZ**1X
XXXX**ZZZZZZZ*ZXX
X1X11Z*Z*Z*ZZ*1XX
XXX1X******ZZZXXX
1XX1XX*Z*ZZ*Z1XX1
XXXXXXXX*ZZ11X11X
.11XXXXX11111X11.
.X1XXX1X11111XXX.
..1XX1XXXXX1111..
...XXXXXXXXXXX...
....X11111X1X....
.......XXXX......
```

`yield` is good die over *present* die (good + defect); the denominator
excludes overhang, though a legal region now always has zero of it, so in
practice `yield` is just good divided by the 93 mask sites.

Because the alphabet is lossless, **the output is valid input**: re-running the
tool on its own report reproduces it byte for byte, and the recorded region can
be recovered with `WaferMap::marked_region()`.

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
that isn't exactly 17 characters wide is dropped before the grid is parsed.
A leading line that does happen to be 17 characters wide is left alone and
runs through normal validation instead, since at that width it can't be told
apart from a genuinely malformed first grid row.

Marks in the input that match no legal mask placement are overwritten by the
run's result, but the tool **warns first** rather than discarding a hand edit
silently.

## Ties and edge cases

- **Ties** are broken in favor of the earlier placement in row-major order.
- A 200mm region must land **entirely on present die**: any placement where
  the mask would hang off the wafer edge onto an absent (`.`) site is not
  legal and is never considered, however many good die it covers. If *no*
  placement anywhere avoids overhang — the wafer's present-die area is
  smaller than the mask everywhere it could sit — there is no legal region at
  all, and the tool reports an error instead of a result.
- A **blank line inside the grid** is an error, not something to silently skip
  — dropping it would shift every later row and yield a confidently wrong
  answer. Blank lines around the grid, CRLF endings, and trailing spaces are
  all tolerated.
- Row and column numbers in **error messages** are 1-based, matching a text
  editor. Row/col in the output header and JSON are 0-based grid indices.

## Usage

```
usage: yield_max [options] <input_path> [output_path]

  --json         Machine-readable JSON on stdout.
  -h, --help     Show help.
```

Output defaults to `<input>_optimal.txt` beside the input; `-` writes the
report to stdout. The tool refuses to overwrite its own input file.

`--json` is the interface for other programs — don't parse the ASCII art:

```json
{"version":2,
 "best":{"row":0,"col":4,"good":62,"defect":31,"overhang":0,"sites":93,"yield":0.6667},
 "mask_sites":93,"output":"wafer_optimal.txt"}
```

If no placement anywhere avoids overhang, the tool exits with an error instead
(nonzero exit code, message on stderr) rather than emitting JSON or a report.

## Layout

- `yield_max-core/` — dependency-free solver and file format. Single source of
  truth for the mask shape (`MASK_TEMPLATE`).
- `yield_max-cli/` — command-line frontend.
- `yield_max-wasm/` — `wasm-bindgen` wrapper; also re-exports the mask via
  `mask_rows()` so the web UI cannot drift out of sync with the solver.
- `index.html` — web frontend (file upload, visualization, report download).

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd yield_max-wasm && wasm-pack build --target web   # pkg/ is checked in
```
