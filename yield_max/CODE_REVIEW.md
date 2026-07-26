# Code Review — yield_max (commit ad5b067)

> **Status: resolved in the follow-up commit.** Findings 1–10 have all been
> addressed by the version-2 output format work. See `README.md` for the new
> format and the rationale. This document is kept as the record of the review;
> the findings below describe the code *as reviewed*, not as it now stands.


Reviewed: `yield_max-core`, `yield_max-cli`, `yield_max-wasm`, `index.html`, fixtures.

## Verdict

Ships and is correct. The core algorithm, mask, CLI, and web UI all agree, and I
independently reproduced the sample answer (**63 good die at row 0, col 5**) with a
throwaway Python brute force. `cargo test --workspace` → 6/6 pass. `wasm-pack build
--target web` reproduces the checked-in `pkg/` byte-for-byte (only a generated
`.gitignore` differs, correctly not committed).

> **Correction.** That byte-for-byte claim was verified only on this machine.
> The `.wasm` embeds absolute paths (`$CARGO_HOME`, the rustc commit hash) in
> panic metadata, so it is *not* reproducible across machines — a CI job built
> on that assumption failed on the first GitHub runner. See `CLAUDE.md` for
> what is checked instead.

The web UI loads, analyzes, and renders the region outline correctly in a real
browser.

Nothing here is a shipping blocker. Findings below are ordered by value.

## Correctness verification performed

- Mask constant in `yield_max-core/src/lib.rs` diffed programmatically against the
  README spec block — identical.
- Exhaustive placement scan (49 placements) reproduced independently: top result
  (63, r0, c5), runner-up (62, r0, c4). No tie at the max, so the documented
  first-wins tie-break is untested by the fixture but also unexercised.
- CLI end-to-end on `test_wafer.txt` produces the expected marked map; 93 mask
  cells, of which the winning placement overhangs exactly one absent (`.`) cell at
  (0,12), which is correctly left as `.`.
- Error paths: missing args, unparseable input, and re-feeding a marked (`Z`) file
  all exit 1 with a clear message.

## Findings

### 1. `Z` output is not re-parseable (medium — design, decide intentionally)

`mark_region` emits `Z`, but `WaferMap::parse` rejects `Z`. Feeding a tool's own
output back in fails:

```
error: failed to parse out.txt: row 0, col 8: invalid character 'Z' ...
```

That may be exactly what you want (output is a report, not an input), but it should
be a deliberate choice. If pipelining is ever desired, accept `Z` on input and treat
it as good-or-defect-unknown — or better, don't; just document the one-way contract
in the README.

### 2. Lossy marking: `1` and `X` both become `Z` (medium — information loss)

`mark_region` overwrites good and defect die with the same character, so the output
file cannot tell you *why* the region won. The README does ask for exactly this, so
it is spec-compliant. Worth raising with whoever owns the spec: a variant using two
characters (e.g. `Z` for good-in-region, `z` for defect-in-region) would make the
artifact far more useful for yield analysis, at zero algorithmic cost.

Also note the docstring says "every present die ('1' or 'X')" — accurate — but the
README says "all die in the optimal 200mm region", which literally includes `.`
cells under the mask overhang. The implementation's choice (leave `.` alone) is the
right one; the deviation from a literal reading of the spec deserves a README note
so a future reader doesn't "fix" it.

### 3. WASM object leak in the browser (medium — real, small)

`index.html` calls `analyze_wafer(...)` and never calls `analysis.free()`. Every
`AnalysisResult` is a wasm-bindgen handle backed by linear memory; each Analyze
click leaks one, and the marked-map `String` clone with it. Cheap fix:

```js
const analysis = analyze_wafer(input.value);
try { /* read analysis.row / .col / .good_die_count / .marked_map */ }
finally { analysis.free(); }
```

The generated `.d.ts` already exposes `[Symbol.dispose]()`, so `using analysis =
analyze_wafer(...)` also works if you're happy requiring explicit resource
management support.

Alternatively, sidestep it: have `analyze_wafer` return a plain JS object via
`serde-wasm-bindgen` or a `js_sys::Object`, and there is no handle to free.

### 4. The mask is defined twice (medium — drift risk)

`MASK_TEMPLATE` in `yield_max-core/src/lib.rs` and `MASK` in `index.html` are
independent copies, guarded only by a "must stay in sync" comment. If they diverge,
the reported count and the highlighted outline silently disagree — the worst kind of
bug, because the UI looks authoritative. Export the mask from WASM instead:

```rust
#[wasm_bindgen]
pub fn mask_rows() -> Vec<JsValue> { /* MASK_TEMPLATE mapped to JsValue */ }
```

...and have the JS build its outline from that. Single source of truth, and the
comment can go away.

The sample wafer is duplicated three times as well (`test_wafer.txt`, the `SAMPLE`
const in the core tests, the `SAMPLE` const in `index.html`). Lower stakes — a stale
copy just makes a test less representative — but the core test could
`include_str!("../../test_wafer.txt")` and drop one copy for free.

### 5. Clippy is not clean (low)

Four `needless_range_loop` warnings on the `dr`/`dc` loops in `find_best_region` and
`mark_region`. The indexed form is arguably clearer here given the parallel indexing
into both `mask` and `map`, so the right fix is probably not to rewrite the loops but
to silence them deliberately:

```rust
#[allow(clippy::needless_range_loop)] // parallel indexing into mask and wafer
```

Either way, get to a zero-warning build so future real warnings aren't lost in noise.
Consider a `#![deny(warnings)]`-in-CI posture, or at minimum add `cargo clippy
--workspace --all-targets -- -D warnings` to whatever passes for CI in this repo.

### 6. Dead `best_found` flag (low)

```rust
let mut best_found = false;
...
if !best_found || good > best.good_die_count {
```

`best` is initialized to `(0, 0, 0)` and the loop always executes at least once, so
`best_found` can never change the outcome: on the first iteration either `good > 0`
(taken by the second clause) or `good == 0` (and `best` already equals that
placement). Delete the flag; the `if good > best.good_die_count` form is equivalent
and states the tie-break rule more plainly.

### 7. CLI ergonomics (low)

- Extra arguments beyond the second are silently ignored. Reject them.
- No `--help` / `-h`; `yield_max --help` treats `--help` as an input path and fails
  with a confusing "failed to read --help" instead of usage.
- Passing the same path for input and output silently overwrites the source wafer
  map. A guard (compare canonicalized paths) or at least a warning would be kind.
- `default_output_path`'s `unwrap_or("wafer")` fallback handles non-UTF-8 stems by
  discarding the original name entirely; a path like `/tmp/wafer.txt/` would produce
  `wafer_optimal.txt` in an unexpected place. Extremely marginal, but if you'd rather
  fail loudly than guess, return an error there.

### 8. Parser is slightly too lenient in one spot, strict in another (low)

- Blank lines are filtered *anywhere*, not just at the ends, so a wafer map with a
  stray blank line in the middle parses happily and silently shifts rows. I verified
  this: a file with a blank line after row 0 still reports 63 at (0,5) rather than
  erroring. Prefer trimming only leading/trailing blank lines.
- `trim_end_matches('\r')` handles CRLF, good — but trailing spaces are not trimmed
  and produce a `WrongRowLength` error. Given the input is hand-editable ASCII art,
  trailing whitespace is a plausible accident worth tolerating.
- Row/col in error messages are 0-based and undocumented as such. For a
  human-facing tool over a text file, 1-based line numbers match what an editor
  shows.

### 9. Test coverage gaps (low)

Current tests are good — they cover parse success, all three parse errors, the known
optimum, and a full cell-by-cell assertion that `mark_region` touches exactly the
mask footprint. That last one is genuinely well written. Missing:

- A tie-break test. The docstring promises first-in-row-major-order wins; nothing
  enforces it. An all-`1` wafer makes several placements tie and pins the behavior.
- An all-`.` wafer (count 0) and an all-`1` wafer (count 93, = the mask's cell
  count) as boundary cases.
- No tests at all in `yield_max-wasm` or `yield_max-cli`. The wasm layer is a thin
  adapter, but `default_output_path` is pure logic that deserves a unit test —
  including the no-parent and no-extension cases.
- `wasm-bindgen-test` would let you cover the error-to-`JsValue` path headlessly.

### 10. Minor web UI notes (low)

- `await init()` at module top level means any init failure produces an unhandled
  rejection and a page that looks fine but where Analyze does nothing. Wrap it and
  surface a visible error.
- The Analyze button is live before init resolves. Disable it until ready.
- No file input and no download button, so the web version can't actually consume or
  produce the *files* the README centers on — it's copy/paste only. A
  `<input type="file">` plus a Blob download would close the gap with the CLI and is
  maybe 15 lines.
- `errorBox` is a bare `<div>`; adding `role="alert"` would announce parse failures
  to screen readers. The grid itself is nicely done — `role="grid"`/`gridcell`,
  per-cell `aria-label`, keyboard focusability, and tooltips on focus as well as
  hover. That's better accessibility than most tools of this size.
- Cell colors are the sole encoding of good vs. defect, which is a problem for
  red/green color blindness — the two most important states are exactly the classic
  confusion pair. The tooltips and aria-labels mitigate it, but a shape or character
  overlay would make the map readable at a glance for everyone.

## What's good

Worth stating plainly, because it's most of the code:

- Clean three-crate split; the core is dependency-free and the CLI and WASM layers
  are thin adapters over it. Adding a third frontend would be trivial.
- `ParseError` is a real enum implementing `Display` + `std::error::Error`, with
  positional detail — not a `String`. The WASM layer reuses it via `to_string()`
  instead of reimplementing messages.
- `main` returns `ExitCode` and routes all failures through one `run() -> Result`,
  so exit codes and stderr are consistent.
- The checked-in `pkg/` is genuinely reproducible from source, which is the main
  hazard of the "commit the wasm build output" pattern this monorepo uses.
- Comments explain *why* (the tie-break rule, the `.`-overhang decision, the
  cross-language sync requirement, the palette rationale) rather than restating code.

## Suggested order of work

1. Free the `AnalysisResult` handle (#3) — smallest real bug.
2. Single-source the mask across Rust and JS (#4) — highest future-bug prevention.
3. Get clippy to zero and delete `best_found` (#5, #6) — five minutes.
4. Tie-break and boundary tests, plus `default_output_path` tests (#9).
5. Decide and document the `Z` round-trip and `.`-overhang contracts (#1, #2).
6. CLI `--help` and arg validation (#7); file upload/download in the UI (#10).
