// section oracle:
//! The Radium oracle: `DECISIONS.md` made runnable (D48, D93).
//!
//! It reads a Radium program, refuses what the compiler would refuse, and runs the rest with every check site
//! checked as it runs, as in mode 2 (D9). It proves nothing; proving is Geiger's job. Whatever the design hasn't
//! decided is reported as not decided, never guessed.

mod ast;
mod binding;
mod checker;
mod interpreter;
mod lexer;
mod outcome;
mod parser;
mod rules;

pub use crate::outcome::Outcome;

use crate::interpreter::Stop;

/// Reads, checks and runs one Radium program, and says how it went.
#[must_use]
pub fn run_source(source: &str) -> Outcome {
    let checked = lexer::tokenize(source)
        .and_then(parser::parse)
        .and_then(|program| checker::check(&program).map(|()| program));
    let program = match checked {
        Ok(program) => program,
        Err(problem) => return problem.into(),
    };
    let (result, output) = interpreter::run(&program);
    match result {
        Ok(()) => Outcome::Runs { output },
        Err(Stop::CheckFails { line, message }) => Outcome::CheckFails {
            line,
            message,
            output,
        },
        Err(Stop::GaveUp { line, message }) => Outcome::GaveUp {
            line,
            message,
            output,
        },
        Err(Stop::Fault(message)) => Outcome::OracleFault { message },
    }
}
// end section oracle
