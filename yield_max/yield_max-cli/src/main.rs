use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use yield_max_core::{
    find_best_region_with, mask_site_count, render_report, BestRegion, Grade, TieBreak, WaferMap,
    MAX_INPUT_BYTES,
};

const USAGE: &str = "\
usage: yield_max [options] <input_path> [output_path]

Finds the highest-yielding placement of the 200mm region on a 300mm wafer
map and writes a marked copy of the map.

If output_path is omitted it defaults to <input>_optimal.txt alongside the
input. Use '-' as the output path to write the report to stdout instead.

The region chosen is the one covering the most grade-4 ('4') die. Good die
are graded 1..4; all four count as good, but grade 4 is what is maximized.

Options:
  --tiebreak=P   How to settle a tie on the grade-4 count (default: grade):
                   grade  prefer the better remaining grades (3, then 2, then 1)
                   total  prefer the most good die overall
                 Recorded in the report header; when re-run on a previous
                 report, the header's policy is used unless this flag says
                 otherwise.
  --json         Emit machine-readable JSON on stdout (including runners-up)
                 instead of the human summary.
  -h, --help     Show this help.";

#[derive(Debug)]
struct Args {
    input: PathBuf,
    output: Option<PathBuf>,
    json: bool,
    /// `None` means "not asked for", which is distinct from the default: it is
    /// what lets a previous report's header supply the policy instead.
    tie_break: Option<TieBreak>,
}

fn parse_args(argv: impl Iterator<Item = String>) -> Result<Option<Args>, String> {
    let mut json = false;
    let mut tie_break = None;
    let mut positional: Vec<String> = Vec::new();

    for arg in argv {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--json" => json = true,
            other if other.starts_with("--tiebreak=") => {
                let value = other.trim_start_matches("--tiebreak=");
                // A misspelled policy silently falling back to the default
                // would answer a different question than the one asked.
                let parsed = value
                    .parse::<TieBreak>()
                    .map_err(|e| format!("{e}\n\n{USAGE}"))?;
                tie_break = Some(parsed);
            }
            // A lone "-" is a legal output path, so only reject longer flags.
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("unknown option '{other}'\n\n{USAGE}"));
            }
            other => positional.push(other.to_string()),
        }
    }

    let mut positional = positional.into_iter();
    let input = positional
        .next()
        .ok_or_else(|| format!("missing input path\n\n{USAGE}"))?;
    let output = positional.next();
    // Silently ignoring surplus arguments hides typos; reject them.
    let extra: Vec<String> = positional.collect();
    if !extra.is_empty() {
        return Err(format!(
            "unexpected extra argument(s): {}\n\n{USAGE}",
            extra.join(", ")
        ));
    }

    Ok(Some(Args {
        input: PathBuf::from(input),
        output: output.map(PathBuf::from),
        json,
        tie_break,
    }))
}

fn default_output_path(input_path: &Path) -> PathBuf {
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("wafer");
    let file_name = format!("{stem}_optimal.txt");
    match input_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(file_name),
        _ => PathBuf::from(file_name),
    }
}

/// True if both paths resolve to the same existing file. Used to avoid
/// clobbering the user's source wafer map with the report.
fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn json_placement(p: &BestRegion) -> String {
    // `good` keeps its version-2 meaning (all grades) so a consumer reading it
    // is not silently given a smaller number; the breakdown is additive.
    let by_grade: Vec<String> = Grade::BEST_FIRST
        .iter()
        .map(|g| format!(r#""{}":{}"#, g.number(), p.stats.grade(*g)))
        .collect();
    format!(
        r#"{{"row":{},"col":{},"good":{},"good_by_grade":{{{}}},"defect":{},"overhang":{},"sites":{},"yield":{:.4}}}"#,
        p.row,
        p.col,
        p.stats.good_total(),
        by_grade.join(","),
        p.stats.defect,
        p.stats.overhang,
        p.stats.sites(),
        p.stats.yield_fraction(),
    )
}

fn json_report(best: &BestRegion, tie_break: TieBreak, output: Option<&Path>) -> String {
    let output_field = match output {
        Some(p) => format!(r#""{}""#, p.display().to_string().replace('"', "\\\"")),
        None => "null".to_string(),
    };
    format!(
        r#"{{"version":3,"tiebreak":"{}","best":{},"mask_sites":{},"output":{}}}"#,
        tie_break,
        json_placement(best),
        mask_site_count(),
        output_field,
    )
}

/// Which tie-break policy to use, and where it came from -- the latter is worth
/// reporting, since a policy inherited from the input file is not obvious from
/// the command line.
fn resolve_tie_break(args: &Args, map: &WaferMap) -> Result<(TieBreak, &'static str), String> {
    match (args.tie_break, map.header_tie_break()) {
        // An explicit flag that contradicts the input's own header asks us to
        // re-solve a finished report under a different policy, and the result
        // would be indistinguishable from that report on sight. Refuse rather
        // than pick one silently.
        (Some(flag), Some(Ok(header))) if flag != *header => Err(format!(
            "--tiebreak={flag} contradicts the tiebreak={header} recorded in this input, \
             which is itself a report; drop the flag to reproduce it, or run \
             --tiebreak={flag} against the original wafer map instead"
        )),
        (Some(flag), _) => Ok((flag, "--tiebreak")),
        (None, Some(Ok(header))) => Ok((*header, "the input's header")),
        // A header naming a policy we don't know is a real problem: we cannot
        // reproduce the file, and quietly using the default would look like we
        // had.
        (None, Some(Err(e))) => Err(format!(
            "{e}\n\nthe input file records a tie-break this build does not know; \
             pass --tiebreak=... to choose one explicitly"
        )),
        (None, None) => Ok((TieBreak::default(), "the default")),
    }
}

fn run() -> Result<(), String> {
    let args = match parse_args(env::args().skip(1))? {
        Some(args) => args,
        None => {
            println!("{USAGE}");
            return Ok(());
        }
    };

    let to_stdout = args.output.as_deref() == Some(Path::new("-"));
    let output_path = if to_stdout {
        None
    } else {
        Some(
            args.output
                .clone()
                .unwrap_or_else(|| default_output_path(&args.input)),
        )
    };

    if let Some(out) = &output_path {
        if same_file(&args.input, out) {
            return Err(format!(
                "refusing to overwrite the input file {}; choose a different output path",
                out.display()
            ));
        }
    }

    // Check the size on disk first: parse() would also reject an oversized
    // input, but only after read_to_string has pulled the whole thing into
    // memory. A 200MB file should cost us a stat, not 200MB of RSS.
    if let Ok(meta) = fs::metadata(&args.input) {
        if meta.is_file() && meta.len() > MAX_INPUT_BYTES as u64 {
            return Err(format!(
                "{} is {} bytes, larger than the {MAX_INPUT_BYTES} byte limit; \
                 a wafer map is 17 rows of 17 characters",
                args.input.display(),
                meta.len()
            ));
        }
    }

    let input = fs::read_to_string(&args.input)
        .map_err(|e| format!("failed to read {}: {e}", args.input.display()))?;

    let map = WaferMap::parse(&input)
        .map_err(|e| format!("failed to parse {}: {e}", args.input.display()))?;

    // Marks that match no legal placement get overwritten by the report, so
    // say so rather than silently discarding what the user hand-edited.
    if map.has_inconsistent_marks() {
        eprintln!(
            "warning: {} contains region marks that match no legal 200mm placement; \
             they will be replaced by this run's result",
            args.input.display()
        );
    }

    let (tie_break, tie_break_source) = resolve_tie_break(&args, &map)?;

    let best = find_best_region_with(&map, tie_break).ok_or_else(|| {
        format!(
            "no 200mm region fits entirely within {}'s wafer with at least one die of \
             clearance from the wafer's edge on every side \
             (its present-die area is too small everywhere it could sit)",
            args.input.display()
        )
    })?;
    let report = render_report(&map, &best, tie_break);

    if let Some(out) = &output_path {
        fs::write(out, &report).map_err(|e| format!("failed to write {}: {e}", out.display()))?;
    }

    if args.json {
        println!("{}", json_report(&best, tie_break, output_path.as_deref()));
        return Ok(());
    }

    if to_stdout {
        print!("{report}");
        return Ok(());
    }

    let s = best.stats;
    println!(
        "Best 200mm region: top-left at (row {}, col {})",
        best.row, best.col
    );
    println!(
        "  {} grade-4 die (the figure being maximized)",
        s.grade(Grade::G4)
    );
    let breakdown: Vec<String> = Grade::BEST_FIRST
        .iter()
        .map(|g| format!("{} grade-{}", s.grade(*g), g.number()))
        .collect();
    println!(
        "  {} good die ({}), {} defect die, {} overhang site(s) of {} total — {:.1}% yield",
        s.good_total(),
        breakdown.join(", "),
        s.defect,
        s.overhang,
        s.sites(),
        s.yield_fraction() * 100.0
    );
    println!("  grade-4 ties broken by {tie_break} (from {tie_break_source})");
    if let Some(out) = &output_path {
        println!("Marked wafer map written to {}", out.display());
    }

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Result<Option<Args>, String> {
        parse_args(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn derives_default_output_path() {
        assert_eq!(
            default_output_path(Path::new("/tmp/wafer.txt")),
            PathBuf::from("/tmp/wafer_optimal.txt")
        );
        assert_eq!(
            default_output_path(Path::new("wafer.txt")),
            PathBuf::from("wafer_optimal.txt")
        );
        // No extension, and no parent component.
        assert_eq!(
            default_output_path(Path::new("wafer")),
            PathBuf::from("wafer_optimal.txt")
        );
    }

    #[test]
    fn parses_input_and_output_positionals() {
        let a = args(&["in.txt", "out.txt"]).unwrap().unwrap();
        assert_eq!(a.input, PathBuf::from("in.txt"));
        assert_eq!(a.output, Some(PathBuf::from("out.txt")));
        assert!(!a.json);
    }

    #[test]
    fn help_short_circuits() {
        assert!(args(&["--help"]).unwrap().is_none());
        assert!(args(&["-h"]).unwrap().is_none());
        // Help wins even alongside other arguments.
        assert!(args(&["in.txt", "--help"]).unwrap().is_none());
    }

    #[test]
    fn json_flag_is_order_independent() {
        assert!(args(&["--json", "in.txt"]).unwrap().unwrap().json);
        assert!(args(&["in.txt", "--json"]).unwrap().unwrap().json);
    }

    #[test]
    fn rejects_missing_input_unknown_flags_and_extra_args() {
        assert!(args(&[]).unwrap_err().contains("missing input path"));
        assert!(args(&["--nope", "in.txt"])
            .unwrap_err()
            .contains("unknown option"));
        assert!(args(&["a", "b", "c"])
            .unwrap_err()
            .contains("unexpected extra argument"));
    }

    #[test]
    fn dash_is_a_valid_stdout_output_path() {
        let a = args(&["in.txt", "-"]).unwrap().unwrap();
        assert_eq!(a.output, Some(PathBuf::from("-")));
    }

    #[test]
    fn json_report_shape() {
        let map = WaferMap::parse(include_str!("../../test_wafer.txt")).unwrap();
        let best = find_best_region_with(&map, TieBreak::Grade).unwrap();
        let json = json_report(&best, TieBreak::Grade, Some(Path::new("out.txt")));
        assert!(json.contains(r#""version":3"#));
        assert!(json.contains(r#""tiebreak":"grade""#));
        // `good` keeps its v2 meaning: every grade, not just grade 1.
        assert!(json.contains(r#""good":57"#));
        assert!(json.contains(r#""good_by_grade":{"4":0,"3":0,"2":0,"1":57}"#));
        assert!(json.contains(r#""overhang":0"#));
        assert!(json.contains(r#""mask_sites":93"#));
        // Exactly one placement is reported: the winner.
        assert_eq!(json.matches(r#""row":"#).count(), 1);
    }

    /// The JSON breakdown must add up to the `good` total it sits beside, on a
    /// wafer that actually uses several grades.
    #[test]
    fn json_grade_breakdown_sums_to_the_good_total() {
        let map = WaferMap::parse(include_str!("../../testdata/grades_mixed.txt")).unwrap();
        let best = find_best_region_with(&map, TieBreak::Grade).unwrap();
        let json = json_placement(&best);

        let total: usize = Grade::BEST_FIRST.iter().map(|g| best.stats.grade(*g)).sum();
        assert_eq!(total, best.stats.good_total());
        assert!(json.contains(&format!(r#""good":{total}"#)), "got: {json}");
        for g in Grade::BEST_FIRST {
            assert!(
                json.contains(&format!(r#""{}":{}"#, g.number(), best.stats.grade(g))),
                "grade {} missing from: {json}",
                g.number()
            );
        }
    }

    #[test]
    fn parses_the_tiebreak_flag() {
        // Absent means "not asked for", which is distinct from the default.
        assert_eq!(args(&["in.txt"]).unwrap().unwrap().tie_break, None);
        assert_eq!(
            args(&["--tiebreak=grade", "in.txt"])
                .unwrap()
                .unwrap()
                .tie_break,
            Some(TieBreak::Grade)
        );
        assert_eq!(
            args(&["in.txt", "--tiebreak=total"])
                .unwrap()
                .unwrap()
                .tie_break,
            Some(TieBreak::Total)
        );
    }

    /// A misspelled policy must fail loudly. Falling back to the default would
    /// answer a different question than the one asked, with no sign of it.
    #[test]
    fn rejects_an_unknown_tiebreak_naming_the_alternatives() {
        for bad in ["--tiebreak=Grade", "--tiebreak=most", "--tiebreak="] {
            let err = args(&[bad, "in.txt"]).unwrap_err();
            assert!(
                err.contains("grade") && err.contains("total"),
                "{bad} produced an unhelpful error: {err}"
            );
        }
    }

    fn resolve(flag: Option<TieBreak>, text: &str) -> Result<(TieBreak, &'static str), String> {
        let args = Args {
            input: PathBuf::from("in.txt"),
            output: None,
            json: false,
            tie_break: flag,
        };
        resolve_tie_break(&args, &WaferMap::parse(text).unwrap())
    }

    /// Precedence: an explicit flag, else the policy recorded in the input's
    /// header, else the default. The middle case is what makes re-running on a
    /// report reproduce it rather than quietly switching policy.
    #[test]
    fn resolves_the_tiebreak_from_flag_then_header_then_default() {
        let plain = include_str!("../../test_wafer.txt");
        assert_eq!(resolve(None, plain).unwrap().0, TieBreak::default());
        assert_eq!(
            resolve(Some(TieBreak::Total), plain).unwrap().0,
            TieBreak::Total
        );

        let report = include_str!("../../testdata/marked_roundtrip.txt");
        let (policy, source) = resolve(None, report).unwrap();
        assert_eq!(policy, TieBreak::Grade);
        assert!(source.contains("header"), "got: {source}");

        // Agreeing with the header is fine; only a contradiction is an error.
        assert_eq!(
            resolve(Some(TieBreak::Grade), report).unwrap().0,
            TieBreak::Grade
        );
    }

    /// Re-running a report under a different policy would overwrite it with the
    /// answer to a different question. Refuse rather than pick one silently.
    #[test]
    fn rejects_a_flag_that_contradicts_the_inputs_header() {
        let report = include_str!("../../testdata/marked_roundtrip.txt");
        let err = resolve(Some(TieBreak::Total), report).unwrap_err();
        assert!(err.contains("contradicts"), "got: {err}");
        assert!(err.contains("tiebreak=grade"), "got: {err}");
    }

    /// A header naming a policy this build doesn't know must not be silently
    /// downgraded to the default: we cannot reproduce that file.
    #[test]
    fn rejects_an_unknown_policy_recorded_in_the_input() {
        let text = format!(
            "# yield_max 9  region=row2,col4 tiebreak=sideways\n{}",
            include_str!("../../test_wafer.txt")
        );
        let err = resolve(None, &text).unwrap_err();
        assert!(err.contains("sideways"), "got: {err}");
        assert!(err.contains("--tiebreak"), "got: {err}");
    }
}
