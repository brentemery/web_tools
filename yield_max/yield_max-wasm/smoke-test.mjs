// Functional smoke test for the committed wasm: loads pkg/ exactly as the
// page does and checks it computes the known answer. This is the property
// that matters -- that the shipped binary works -- which a byte-for-byte
// comparison never actually verified.
import { readFile } from 'node:fs/promises';
import init, { analyze_wafer, mask_rows, mask_sites, legend } from './pkg/yield_max_wasm.js';

const wasm = await readFile(new URL('./pkg/yield_max_wasm_bg.wasm', import.meta.url));
await init({ module_or_path: wasm });

const map = await readFile(new URL('../test_wafer.txt', import.meta.url), 'utf8');
const r = analyze_wafer(map);
const b = r.best;

const checks = [
  ['row', b.row, 0],
  ['col', b.col, 4],
  ['good', b.good, 62],
  ['defect', b.defect, 31],
  ['overhang', b.overhang, 0],
  ['sites', b.sites, 93],
  ['mask_sites()', mask_sites(), 93],
  ['mask_rows() length', mask_rows().length, 11],
];

let failed = 0;
for (const [name, got, want] of checks) {
  if (got !== want) { console.error(`  FAIL ${name}: got ${got}, want ${want}`); failed++; }
}
if (!legend().includes('Z=good')) { console.error('  FAIL legend()'); failed++; }

// The report must round-trip: our own output is valid input.
const report = r.report;
b.free(); r.free();
const r2 = analyze_wafer(report);
if (r2.report !== report) { console.error('  FAIL round-trip: re-running changed the report'); failed++; }
r2.best.free(); r2.free();

// Malformed input must reject, not silently produce something.
try { analyze_wafer('garbage'); console.error('  FAIL: malformed input was accepted'); failed++; }
catch { /* expected */ }

// A wafer with no die anywhere has no legal overhang-free 200mm placement.
try {
  analyze_wafer(Array(17).fill('.'.repeat(17)).join('\n'));
  console.error('  FAIL: an all-absent wafer was accepted');
  failed++;
} catch { /* expected */ }

if (failed) { console.error(`${failed} check(s) failed`); process.exit(1); }
console.log('  committed wasm loads and computes the expected result');
