use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use yield_max_core::{find_best_region, mask_site_count, render_report, BestRegion, WaferMap};

const USAGE: &str = "\
usage: yield_max [options] <input_path> [output_path]

Finds the highest-yielding placement of the 200mm region on a 300mm wafer
map and writes a marked copy of the map.

If output_path is omitted it defaults to <input>_optimal.txt alongside the
input. Use '-' as the output path to write the report to stdout instead.

Options:
  --json         Emit machine-readable JSON on stdout (including runners-up)
                 instead of the human summary.
  -h, --help     Show this help.";

#[derive(Debug)]
struct Args {
    input: PathBuf,
    output: Option<PathBuf>,
    json: bool,
}

fn parse_args(argv: impl Iterator<Item = String>) -> Result<Option<Args>, String> {
    let mut json = false;
    let mut positional: Vec<String> = Vec::new();

    for arg in argv {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--json" => json = true,
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
    format!(
        r#"{{"row":{},"col":{},"good":{},"defect":{},"overhang":{},"sites":{},"yield":{:.4}}}"#,
        p.row,
        p.col,
        p.stats.good,
        p.stats.defect,
        p.stats.overhang,
        p.stats.sites(),
        p.stats.yield_fraction(),
    )
}

fn json_report(best: &BestRegion, output: Option<&Path>) -> String {
    let output_field = match output {
        Some(p) => format!(r#""{}""#, p.display().to_string().replace('"', "\\\"")),
        None => "null".to_string(),
    };
    format!(
        r#"{{"version":2,"best":{},"mask_sites":{},"output":{}}}"#,
        json_placement(best),
        mask_site_count(),
        output_field,
    )
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

    let input = fs::read_to_string(&args.input)
        .map_err(|e| format!("failed to read {}: {e}", args.input.display()))?;

    let map = WaferMap::parse(&input)
        .map_err(|e| format!("failed to parse {}: {e}", args.input.display()))?;

    let best = find_best_region(&map);
    let report = render_report(&map, &best);

    if let Some(out) = &output_path {
        fs::write(out, &report).map_err(|e| format!("failed to write {}: {e}", out.display()))?;
    }

    if args.json {
        println!("{}", json_report(&best, output_path.as_deref()));
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
        "  {} good die, {} defect die, {} overhang site(s) of {} total — {:.1}% yield",
        s.good,
        s.defect,
        s.overhang,
        s.sites(),
        s.yield_fraction() * 100.0
    );
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
        let json = json_report(&find_best_region(&map), Some(Path::new("out.txt")));
        assert!(json.contains(r#""version":2"#));
        assert!(json.contains(r#""good":63"#));
        assert!(json.contains(r#""overhang":1"#));
        assert!(json.contains(r#""mask_sites":93"#));
        // Exactly one placement is reported: the winner.
        assert_eq!(json.matches(r#""row":"#).count(), 1);
    }
}
