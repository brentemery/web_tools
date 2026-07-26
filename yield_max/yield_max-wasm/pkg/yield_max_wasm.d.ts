/* tslint:disable */
/* eslint-disable */

export class AnalysisResult {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly best: Placement;
    /**
     * The full self-describing report: `#` header plus the marked grid.
     */
    readonly report: string;
    /**
     * A non-fatal advisory, or the empty string. Currently set when the
     * input carried region marks that this run will overwrite.
     */
    readonly warning: string;
}

/**
 * Scored placement of the 200mm region, carrying the full breakdown of why
 * it scored as it did rather than just the good-die count.
 */
export class Placement {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly col: number;
    readonly defect: number;
    readonly good: number;
    readonly overhang: number;
    readonly row: number;
    readonly sites: number;
    /**
     * Good die as a fraction of present die, in 0.0..=1.0.
     */
    readonly yield_fraction: number;
}

export function analyze_wafer(input: string): AnalysisResult;

/**
 * The cell-alphabet legend, so the UI never has to restate it.
 */
export function legend(): string;

/**
 * The 200mm mask footprint as `O`/`.` rows. Exported so the web UI can draw
 * the region outline from the same constant the solver uses, instead of
 * keeping a copy that could silently drift out of sync.
 */
export function mask_rows(): string[];

/**
 * Total die sites a 200mm region occupies, wherever it is placed.
 */
export function mask_sites(): number;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_analysisresult_free: (a: number, b: number) => void;
    readonly __wbg_placement_free: (a: number, b: number) => void;
    readonly analysisresult_best: (a: number) => number;
    readonly analysisresult_report: (a: number) => [number, number];
    readonly analysisresult_warning: (a: number) => [number, number];
    readonly analyze_wafer: (a: number, b: number) => [number, number, number];
    readonly legend: () => [number, number];
    readonly mask_rows: () => [number, number];
    readonly mask_sites: () => number;
    readonly placement_col: (a: number) => number;
    readonly placement_defect: (a: number) => number;
    readonly placement_good: (a: number) => number;
    readonly placement_overhang: (a: number) => number;
    readonly placement_row: (a: number) => number;
    readonly placement_sites: (a: number) => number;
    readonly placement_yield_fraction: (a: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
