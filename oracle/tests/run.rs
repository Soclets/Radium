// section oracle.tests:
//! Runs every program in `tests/programs/` and compares the oracle's verdict with the program's own lines:
//!
//! - `// trace D<n>`: the decisions it tests; at least one is required (D18, D48).
//! - `// expect: <verdict>`: exactly one, e.g. `runs` or `refused at line 7` (see `Outcome::verdict`).
//! - `// output: <text>`: one per line the program prints, in order.

use std::fs;
use std::path::Path;

#[test]
fn every_program_matches_its_expectation() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("programs");
    let entries = fs::read_dir(&directory);
    assert!(entries.is_ok(), "can't read {}", directory.display());
    let Ok(entries) = entries else { return };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ra"))
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no test programs in {}",
        directory.display()
    );
    let failures: Vec<String> = paths.iter().filter_map(|path| judge(path).err()).collect();
    assert!(
        failures.is_empty(),
        "{} of {} programs disagree:\n\n{}",
        failures.len(),
        paths.len(),
        failures.join("\n\n")
    );
}

/// Runs one program and compares; `Err` says how it disagreed.
fn judge(path: &Path) -> Result<(), String> {
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let source =
        fs::read_to_string(path).map_err(|error| format!("{name}: can't read it: {error}"))?;
    let mut expected_verdicts = Vec::new();
    let mut expected_output = String::new();
    let mut traces = 0_usize;
    for line in source.lines().map(str::trim) {
        if let Some(verdict) = line.strip_prefix("// expect: ") {
            expected_verdicts.push(verdict.trim().to_owned());
        } else if let Some(text) = line.strip_prefix("// output: ") {
            expected_output.push_str(text);
            expected_output.push('\n');
        } else if line.starts_with("// trace D") {
            traces = traces.saturating_add(1);
        }
    }
    if traces == 0 {
        return Err(format!("{name}: has no `// trace D<n>` line"));
    }
    let [expected_verdict] = expected_verdicts.as_slice() else {
        return Err(format!("{name}: needs exactly one `// expect:` line"));
    };
    let outcome = oracle::run_source(&source);
    if outcome.verdict() != *expected_verdict {
        return Err(format!(
            "{name}: expected `{expected_verdict}`, got `{}`",
            outcome.report()
        ));
    }
    if outcome.output() != expected_output {
        return Err(format!(
            "{name}: expected output\n{expected_output}got\n{}",
            outcome.output()
        ));
    }
    Ok(())
}
// end section oracle.tests
