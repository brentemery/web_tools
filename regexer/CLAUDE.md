# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Context

`regexer` is a planned web-based regex tool, part of the `web_tools` monorepo at `/home/exedev/code/web_tools/`. The project is currently a stub with no source files yet.

## Monorepo Architecture

The `web_tools` monorepo follows a consistent pattern established by the `crosswind` sibling project:

- **Static HTML** — single `.html` file per tool, no build framework
- **Vanilla JavaScript** — ES modules, no npm/bundler required
- **Rust + WebAssembly** (optional) — for compute-intensive logic; compiled via `wasm-bindgen` to a `pkg/` directory that the JS imports directly
- **No Node.js / npm** — tools are static files served directly

When a WASM component is needed, it lives in a subdirectory (e.g., `regexer-wasm/`) with this structure:
```
regexer-wasm/
  Cargo.toml       (crate-type = ["cdylib"], depends on wasm-bindgen)
  src/lib.rs       (#[wasm_bindgen] pub fn exports)
  pkg/             (wasm-pack build output — checked in)
```

The JS frontend imports from `./regexer-wasm/pkg/regexer_wasm.js` and calls `await init()` before using any exported functions.

After adding a new tool, add a link to it in `/home/exedev/code/web_tools/index.html`.

## Build & Test Commands

### Rust/WASM (if a WASM crate is added)

```bash
# Build WASM (run from inside the *-wasm/ directory)
wasm-pack build --target web

# Run Rust unit tests (no browser needed)
cargo test
```

### Serving locally

No build step for the HTML/JS layer. Serve the repo root with any static file server, e.g.:
```bash
python3 -m http.server 8080
```
Then open `http://localhost:8080/regexer/`.
