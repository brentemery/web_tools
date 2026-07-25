use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use yield_max_core::{find_best_region, mark_region, WaferMap};

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

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let input_arg = args
        .next()
        .ok_or_else(|| "usage: yield_max <input_path> [output_path]".to_string())?;
    let input_path = PathBuf::from(input_arg);
    let output_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_path(&input_path));

    let input = fs::read_to_string(&input_path)
        .map_err(|e| format!("failed to read {}: {e}", input_path.display()))?;

    let map = WaferMap::parse(&input)
        .map_err(|e| format!("failed to parse {}: {e}", input_path.display()))?;

    let best = find_best_region(&map);
    let marked = mark_region(&map, &best);

    println!(
        "Best 200mm region: top-left at (row {}, col {}), good die count: {}",
        best.row, best.col, best.good_die_count
    );

    fs::write(&output_path, &marked)
        .map_err(|e| format!("failed to write {}: {e}", output_path.display()))?;

    println!("Marked wafer map written to {}", output_path.display());

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
