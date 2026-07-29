//! Standalone HTML rendering of a solved result.
//!
//! The page mirrors the results panel of the web frontend (`index.html`) --
//! same class names, same colour tokens, same legend -- so a report opened from
//! disk and the same result in the browser read as one design.
//!
//! Two properties are deliberate and worth preserving:
//!
//! - **Self-contained.** Styles are inline, there is no script, and there is no
//!   external reference of any kind. The file is written wherever the caller
//!   asks and has to render from there -- another directory, a ticket
//!   attachment, an offline machine -- so it cannot depend on the web page's
//!   vendored stylesheet or on the network.
//! - **Deterministic.** Nothing in the output varies between runs on the same
//!   input (no timestamp, no host name), so two reports can be diffed and the
//!   renderer can be tested by comparison.

use crate::{
    mask, render_report, BestRegion, Die, Grade, TieBreak, WaferMap, BOARD_SIZE, MASK_SIZE,
};

/// The page's styles, inlined. Ported from `index.html`'s results panel: the
/// `.viz-root` token block, the cell/grade rules, the legend and the stats list
/// are copied as-is so the two views cannot drift apart visually. Two
/// differences are forced by standing alone: the web page's `var(--pico-*)`
/// references are resolved to literals (Pico is not loaded here), and a small
/// base block replaces what Pico's classless reset would have provided.
const STYLE: &str = "\
:root {
  color-scheme: light;
}

body {
  margin: 0 auto;
  max-width: 52rem;
  padding: 2rem 1rem 4rem;
  font-family: system-ui, -apple-system, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
  line-height: 1.5;
  color: #1b1b18;
  background: #ffffff;
}

h1 {
  font-size: 1.75rem;
  margin: 0 0 0.4rem;
}

header p {
  color: #646464;
  margin: 0 0 1.75rem;
}

code,
pre {
  font-family: monospace;
  font-size: 0.9rem;
}

.summary {
  font-size: 1rem;
  margin-bottom: 1.25rem;
}

/* Wafer visualization -- token roles per the dataviz palette (light mode only,
   matching the web page's forced data-theme=\"light\"). Status colors
   (good/critical) and the region-selection accent (categorical slot 1) are used
   verbatim from the documented default palette; nothing here is eyeballed. */
.viz-root {
  --chart-surface: #fcfcfb;
  --text-secondary: #52514e;
  --text-muted: #898781;
  --border-hairline: rgba(11, 11, 11, 0.1);
  --cell-absent: #e1e0d9;
  --cell-good: #0ca30c;
  --cell-defect: #d03b3b;
  --region-accent: #2a78d6;
  /* Grades are one sequential ramp off the good-die hue, not four
     categorical colours: the grades are ordered, so the encoding should
     be too, and a reader who can't separate the steps still has the
     glyph. Grade 4 -- the figure being maximized -- is the darkest and
     so the most prominent. */
  --cell-good-1: #a8dea8;
  --cell-good-2: #6ec96e;
  --cell-good-3: #33ad33;
  --cell-good-4: #0a6b0a;
}

/* Good/defect are the classic red/green confusion pair, so colour alone
   is not a sufficient encoding. Every cell also carries its glyph. */
.wafer-cell::after {
  content: attr(data-glyph);
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: monospace;
  font-size: clamp(0.5rem, 1.1vw, 0.8rem);
  font-weight: 700;
  color: rgba(255, 255, 255, 0.92);
  pointer-events: none;
}

.wafer-cell[data-state='absent']::after {
  color: var(--text-muted);
}

/* The pale end of the grade ramp needs dark text to stay legible. */
.wafer-cell[data-grade='1']::after,
.wafer-cell[data-grade='2']::after {
  color: #0b2f0b;
}

.wafer-viz {
  background: var(--chart-surface);
  border: 1px solid var(--border-hairline);
  border-radius: 0.25rem;
  padding: 1.25rem;
  display: flex;
  flex-wrap: wrap;
  gap: 1.75rem;
  align-items: flex-start;
}

.wafer-grid {
  display: grid;
  grid-template-columns: repeat(17, 1fr);
  width: min(100%, 30rem);
  aspect-ratio: 1 / 1;
  gap: 0;
  flex-shrink: 0;
}

.wafer-cell {
  position: relative;
  aspect-ratio: 1 / 1;
  background: var(--cell-absent);
  border: 3px solid transparent;
  box-sizing: border-box;
  box-shadow: inset 0 0 0 1px #0b0b0b;
}

.wafer-cell[data-state='good'] {
  background: var(--cell-good);
}

.wafer-cell[data-grade='1'] {
  background: var(--cell-good-1);
}
.wafer-cell[data-grade='2'] {
  background: var(--cell-good-2);
}
.wafer-cell[data-grade='3'] {
  background: var(--cell-good-3);
}
.wafer-cell[data-grade='4'] {
  background: var(--cell-good-4);
}

.wafer-cell[data-state='defect'] {
  background: var(--cell-defect);
}

.wafer-cell:hover {
  filter: brightness(1.15);
  z-index: 1;
}

.legend {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  font-size: 0.85rem;
  color: var(--text-secondary);
  min-width: 11rem;
}

.legend-item {
  display: flex;
  align-items: center;
  gap: 0.55rem;
}

.legend-swatch {
  width: 14px;
  height: 14px;
  border-radius: 3px;
  flex-shrink: 0;
  background: var(--cell-absent);
}

.legend-swatch.good-1 {
  background: var(--cell-good-1);
}
.legend-swatch.good-2 {
  background: var(--cell-good-2);
}
.legend-swatch.good-3 {
  background: var(--cell-good-3);
}
.legend-swatch.good-4 {
  background: var(--cell-good-4);
}

.legend-swatch.defect {
  background: var(--cell-defect);
}

.legend-swatch.region {
  background: transparent;
  border: 3px solid var(--region-accent);
}

.stats {
  margin: 0 0 1.25rem;
  padding-left: 1.1rem;
  font-size: 0.95rem;
}

.stats li {
  margin-bottom: 0.2rem;
}

.stats .grade-breakdown {
  list-style: none;
  padding-left: 0.9rem;
  margin: 0.25rem 0 0.35rem;
  font-size: 0.9rem;
  color: #52514e;
}

details.raw-output {
  margin-top: 1.5rem;
}

details.raw-output summary {
  cursor: pointer;
  color: #52514e;
}

.raw-output pre {
  background: #fbfbfa;
  border: 1px solid #cfcec9;
  border-radius: 0.25rem;
  padding: 0.9rem 1.1rem;
  margin-top: 0.75rem;
  white-space: pre;
  overflow-x: auto;
}
";

/// Text-to-markup escaping. Only the source label is genuinely untrusted (a
/// path may contain any of these), but the report body goes through it too:
/// today's cell alphabet happens to be markup-safe, and a renderer that relies
/// on that would break quietly if the alphabet ever grew.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// The die's state as the `data-state` attribute value, which drives its colour.
fn state_name(die: Die) -> &'static str {
    match die {
        Die::Good(_) => "good",
        Die::Defect => "defect",
        Die::Absent => "absent",
    }
}

/// The per-cell description, used for both `title` (a tooltip without needing
/// script) and `aria-label`. Same wording as the web page's cell labels.
fn cell_label(die: Die, row: usize, col: usize, in_region: bool) -> String {
    let what = match die {
        Die::Good(g) => format!("Grade-{} good die", g.number()),
        Die::Defect => "Defect die".to_string(),
        Die::Absent => "Not present".to_string(),
    };
    let suffix = match (in_region, die) {
        (false, _) => "",
        (true, Die::Absent) => " — in region, overhang (no die)",
        (true, _) => " — in selected region",
    };
    format!("Row {row}, col {col}: {what}{suffix}")
}

/// Whether `(row, col)` falls under the mask placed at `region`.
fn in_region(region: &BestRegion, row: usize, col: usize) -> bool {
    row >= region.row
        && row < region.row + MASK_SIZE
        && col >= region.col
        && col < region.col + MASK_SIZE
        && mask()[row - region.row][col - region.col]
}

/// Renders the result as a standalone HTML page: the marked wafer grid, the
/// headline numbers, the legend, and the full text report embedded verbatim.
///
/// `source` names the input the result came from (a path, typically) and is
/// shown in the title and subtitle; pass `None` when there is no meaningful
/// name for it. The output has no external dependencies and no run-varying
/// content -- see the module docs.
pub fn render_html(
    map: &WaferMap,
    region: &BestRegion,
    tie_break: TieBreak,
    source: Option<&str>,
) -> String {
    let s = &region.stats;
    let source = source.map(escape_html);

    let mut out = String::with_capacity(48 * 1024);

    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\" />\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n");
    match &source {
        Some(src) => out.push_str(&format!("<title>Yield Max — {src}</title>\n")),
        None => out.push_str("<title>Yield Max</title>\n"),
    }
    out.push_str("<style>\n");
    out.push_str(STYLE);
    out.push_str("</style>\n</head>\n<body>\n");

    out.push_str("<header>\n<h1>Yield Max</h1>\n");
    match &source {
        Some(src) => out.push_str(&format!(
            "<p>Highest-yielding 200mm region of <code>{src}</code>, \
             the placement covering the most grade-4 die.</p>\n"
        )),
        None => out.push_str(
            "<p>Highest-yielding 200mm region, the placement covering the most \
             grade-4 die.</p>\n",
        ),
    }
    out.push_str("</header>\n<main>\n");

    out.push_str(&format!(
        "<p class=\"summary\">Best 200mm region: top-left at (row <strong>{}</strong>, \
         col <strong>{}</strong>), with grade-4 ties broken by <strong>{}</strong>.</p>\n",
        region.row, region.col, tie_break,
    ));

    // Same list, same order as the web page's stats panel: the number being
    // maximized first, then the totals it was chosen from.
    out.push_str("<ul class=\"stats\">\n");
    out.push_str(&format!(
        "<li><strong>{}</strong> grade-4 die — the figure being maximized</li>\n",
        s.grade(Grade::G4),
    ));
    out.push_str(&format!(
        "<li><strong>{}</strong> good die in total\n<ul class=\"grade-breakdown\">\n",
        s.good_total(),
    ));
    for g in Grade::BEST_FIRST {
        out.push_str(&format!("<li>{} grade-{}</li>\n", s.grade(g), g.number()));
    }
    out.push_str("</ul>\n</li>\n");
    out.push_str(&format!(
        "<li><strong>{}</strong> defect die</li>\n",
        s.defect
    ));
    out.push_str(&format!(
        "<li><strong>{}</strong> overhang site(s) — mask area falling off the \
         wafer edge</li>\n",
        s.overhang,
    ));
    out.push_str(&format!(
        "<li><strong>{:.1}%</strong> yield across <strong>{}</strong> die sites</li>\n",
        s.yield_fraction() * 100.0,
        s.sites(),
    ));
    out.push_str("</ul>\n");

    out.push_str("<div class=\"viz-root\">\n<div class=\"wafer-viz\">\n");
    out.push_str(
        "<div class=\"wafer-grid\" role=\"grid\" \
         aria-label=\"Wafer die map with selected 200mm region\">\n",
    );
    for r in 0..BOARD_SIZE {
        // `display: contents` keeps one 17x17 CSS grid while the row wrappers
        // give assistive tech real rows to walk.
        out.push_str("<div role=\"row\" style=\"display: contents\">\n");
        for c in 0..BOARD_SIZE {
            let die = map.get(r, c);
            let region_cell = in_region(region, r, c);

            out.push_str("<div class=\"wafer-cell\" role=\"gridcell\"");
            out.push_str(&format!(" data-state=\"{}\"", state_name(die)));
            // Only good die carry a grade; the attribute drives the colour
            // ramp, so it must be absent (not "0") for everything else.
            if let Some(g) = die.grade() {
                out.push_str(&format!(" data-grade=\"{}\"", g.number()));
            }
            out.push_str(&format!(" data-region=\"{region_cell}\""));
            // Every glyph in the v3 alphabet is markup-safe, but escape anyway
            // so that stays true by construction rather than by luck.
            out.push_str(&format!(
                " data-glyph=\"{}\"",
                escape_html(&die.to_char(region_cell).to_string()),
            ));

            // The region outline is drawn per cell on the edges that face out
            // of the region, so the mask's stepped outline is traced exactly.
            if region_cell {
                let mut style = String::new();
                if !in_region(region, r.wrapping_sub(1), c) {
                    style.push_str("border-top-color: var(--region-accent);");
                }
                if !in_region(region, r + 1, c) {
                    style.push_str("border-bottom-color: var(--region-accent);");
                }
                if !in_region(region, r, c.wrapping_sub(1)) {
                    style.push_str("border-left-color: var(--region-accent);");
                }
                if !in_region(region, r, c + 1) {
                    style.push_str("border-right-color: var(--region-accent);");
                }
                if !style.is_empty() {
                    out.push_str(&format!(" style=\"{style}\""));
                }
            }

            let label = escape_html(&cell_label(die, r, c, region_cell));
            out.push_str(&format!(
                " title=\"{label}\" aria-label=\"{label}\"></div>\n"
            ));
        }
        out.push_str("</div>\n");
    }
    out.push_str("</div>\n");

    // Legend rows for the grades come from the solver's grade set and its own
    // glyph mapping, so a change to either shows up here without an edit.
    out.push_str("<div class=\"legend\">\n");
    for g in Grade::BEST_FIRST {
        let die = Die::Good(g);
        out.push_str(&format!(
            "<div class=\"legend-item\"><span class=\"legend-swatch good-{n}\"></span>\
             Grade-{n} die (<code>{}</code>, <code>{}</code> in region)</div>\n",
            die.to_char(false),
            die.to_char(true),
            n = g.number(),
        ));
    }
    out.push_str(&format!(
        "<div class=\"legend-item\"><span class=\"legend-swatch defect\"></span>\
         Defect die (<code>{}</code>, <code>{}</code> in region)</div>\n",
        Die::Defect.to_char(false),
        Die::Defect.to_char(true),
    ));
    out.push_str(&format!(
        "<div class=\"legend-item\"><span class=\"legend-swatch\"></span>\
         Not present (<code>{}</code>, <code>{}</code> in region)</div>\n",
        Die::Absent.to_char(false),
        Die::Absent.to_char(true),
    ));
    out.push_str(
        "<div class=\"legend-item\"><span class=\"legend-swatch region\"></span>\
         Selected 200mm region</div>\n",
    );
    out.push_str("</div>\n</div>\n</div>\n");

    // The same bytes as the .txt report, so this page is enough to recover the
    // machine-readable artifact if it is separated from its sibling file.
    out.push_str("<details class=\"raw-output\">\n");
    out.push_str("<summary>Show raw marked wafer text</summary>\n<pre>");
    out.push_str(&escape_html(&render_report(map, region, tie_break)));
    out.push_str("</pre>\n</details>\n");

    out.push_str("</main>\n</body>\n</html>\n");
    out
}
