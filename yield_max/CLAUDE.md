# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Context

`yield_max` is a Rust tool, part of the `web_tools` monorepo at `/home/exedev/git/web_tools/`. `README.md` is the spec; the workspace is `yield_max-core/` (solver), `yield_max-cli/`, and `yield_max-wasm/`, with `index.html` as the web frontend.

## What it does

Given a 300mm wafer map as an ASCII text file, find the 200mm sub-region capturing the most grade-4 die and report the result.

- The 300mm wafer is a 17x17 grid of single characters, one per die:
  - `.` = no die present at that position
  - `X` = defective die
  - `1`, `2`, `3`, `4` = a good die, of that **grade** (bin/speed grade)
- All four digits are good die and all four count toward yield. `1` is grade 1, not a deprecated "ungraded" spelling, so every map written before grades existed is still valid input with an unchanged answer.
- The 200mm region is an 11x11 mask (a fixed circular-ish shape, see `O` cells in `README.md`) that gets overlaid somewhere on the 300mm grid.
- The tool searches all *legal* placements of the mask (no overhang onto absent sites, plus at least one die of clearance from the wafer's true edge) and picks the one covering the most **grade-4** die.
- A tie on the grade-4 count is settled by `--tiebreak`: `grade` (default) prefers the better remaining grades, `total` prefers the most good die overall. Placements neither can separate fall back to row-major-first. Both policies reduce to "most good die" on a grade-1-only wafer, which is why no pre-existing fixture changed its answer.
- Output: the per-grade good counts for the optimal region, plus a new version of the input wafer text file with the region marked. In-region good die become `A`..`D` by grade, defects `*`, absent sites `-`. Version 2's `Z` is still accepted on input (as in-region grade 1) but never emitted.
- **Version 4 labels the axes**: rows are lettered `A`.. top to bottom with `N` skipped (`ROW_LABELS` in core is the only place that rule lives), columns numbered 1..17, and a site is named `<letter><number>` — `A1`, `H10`, `R17`. Reports emit the column numbers as two `#` comment lines and prefix each grid row with its letter and a space; both are read back, so a report is still valid input, and an unlabeled 17-wide grid (anything written before v4) parses unchanged. A label that disagrees with its position, or a partially labeled file, is an error — it means a row was inserted, dropped or reordered.
- The result also names the region's **center die**: the mask's middle site, offset (5,5) from the top-left corner, which is always a present site. It appears as `center=` in the header, in the CLI summary, in JSON, in the HTML report and (ringed on the grid) in the web UI. Labels are always *additive*: the 0-based `row=`/`col=` numbers are unchanged everywhere, so nothing that does arithmetic has to learn the notation.
- The report header records `tiebreak=`, and the parser reads it back, so re-running on a report reproduces it rather than silently switching policy.
- The CLI also writes an **HTML report** of every run (`<output>.html`, or `<input>_optimal.html` when the text report has no output path), rendered by `render_html` in core. It mirrors the web frontend's results panel — grid, legend, stats, the text report in a `<details>` — and must stay **self-contained** (inline CSS, no script, no external reference: it renders from wherever it was written) and **deterministic** (no timestamps, so two runs are byte-identical). Both properties are asserted in `yield_max-core/src/tests.rs` and in CI's end-to-end step. The styling is a deliberate copy of `index.html`'s; the *data* (grades, glyphs, legend, mask, stats) comes from core so only cosmetics can drift.
- `test_wafer.txt` is a real (ungraded) sample input; `testdata/` holds the fixture set, each carrying its own expected answer in a `# expect:` header.

## Monorepo Architecture

`web_tools` is a collection of small, independent tools, each in its own top-level directory (see sibling `crosswind/` and `regexer/`). The established pattern:

- **Static HTML** — single `.html` file per tool, no build framework, no Node.js/npm
- **Vanilla JavaScript** — ES modules where JS is needed
- **Rust + WebAssembly** (optional) — for compute-intensive logic; compiled via `wasm-bindgen` to a `pkg/` directory that the JS imports directly

When a WASM component is needed, it lives in a subdirectory named `<tool>-wasm/`:
```
yield_max-wasm/
  Cargo.toml       (crate-type = ["cdylib"], depends on wasm-bindgen)
  src/lib.rs        (#[wasm_bindgen] pub fn exports)
  pkg/              (wasm-pack build output — checked in)
```
The JS frontend imports from `./yield_max-wasm/pkg/yield_max_wasm.js` and calls `await init()` before using any exported functions. See `crosswind/crosswind-wasm/` for a working example of this layout (`Cargo.toml`, `src/lib.rs`, checked-in `pkg/`).

After adding the tool's HTML entry point, add a link to it in `/home/exedev/git/web_tools/index.html`.

## Build & Test Commands

### Rust/WASM (once the crate exists)

```bash
# Build WASM (run from inside the *-wasm/ directory)
wasm-pack build --target web

# Functional check of the committed wasm (needs node)
node smoke-test.mjs

# Run Rust unit tests (no browser needed)
cargo test
```

### On comparing the committed `pkg/`

`pkg/` is checked in so the page works from a plain static server. CI keeps it
honest, but **does not compare `yield_max_wasm_bg.wasm` byte-for-byte**: the
binary embeds absolute paths (`$CARGO_HOME`, the rustc commit hash) in panic
metadata, so it differs between machines while being functionally identical.
A byte comparison fails on every runner regardless of whether anything is
wrong.

Instead CI checks the two things that are actually meaningful and are stable
across machines:

1. `smoke-test.mjs` loads the committed wasm and asserts it computes the known
   answer, round-trips its output, and rejects malformed input.
2. The generated JS/TS bindings (`.js`, `.d.ts`, `package.json`) are compared
   exactly -- they contain no machine-specific content, so a stale `pkg/` with
   a drifted API surface is caught there.

### Serving locally

No build step for the HTML/JS layer. Serve the repo root with any static file server, e.g.:
```bash
python3 -m http.server 8080
```
Then open `http://localhost:8080/yield_max/`.
