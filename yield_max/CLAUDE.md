# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Context

`yield_max` is a planned Rust tool, part of the `web_tools` monorepo at `/home/exedev/git/web_tools/`. The project is currently a stub containing only `README.md` (the spec) and `test_wafer.txt` (a sample input file) — no source code yet.

## What it does

Given a 300mm wafer map as an ASCII text file, find the highest-yielding 200mm sub-region and report the result.

- The 300mm wafer is a 17x17 grid of single characters, one per die:
  - `.` = no die present at that position
  - `X` = defective die
  - `1` = good die
- The 200mm region is an 11x11 mask (a fixed circular-ish shape, see `O` cells in `README.md`) that gets overlaid somewhere on the 300mm grid.
- The tool must search over all valid placements of the 11x11 mask within the 17x17 grid, count good (`1`) die covered by the mask at each placement, and pick the placement that maximizes that count.
- Output: the count of good die in the optimal 200mm region, plus a new version of the input wafer text file with every die inside the winning region marked `Z` (overwriting whatever character — `.`, `X`, or `1` — was there).
- `test_wafer.txt` is a real sample input matching this format and should be used as the manual/integration test fixture.

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
