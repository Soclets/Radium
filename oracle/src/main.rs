// section oracle.command:
//! `oracle run <program.ra>`: runs one Radium program through the oracle.
//!
//! The program's output goes to stdout and the verdict to stderr. The exit status is 0 only when the program ran to
//! the end of `fn.main`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let (Some(command), Some(path), None) = (arguments.next(), arguments.next(), arguments.next())
    else {
        eprintln!("usage: oracle run <program.ra>");
        return ExitCode::from(2);
    };
    if command != "run" {
        eprintln!("usage: oracle run <program.ra>");
        return ExitCode::from(2);
    }
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("can't read {path}: {error}");
            return ExitCode::from(2);
        }
    };
    let outcome = oracle::run_source(&source);
    print!("{}", outcome.output());
    eprintln!("{}", outcome.report());
    if matches!(outcome, oracle::Outcome::Runs { .. }) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
// end section oracle.command
