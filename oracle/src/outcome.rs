// section oracle.outcome:
//! What judging a program can end in, and the problems found before it runs.

/// How the oracle judged a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The program ran to the end of `main`.
    Runs {
        /// Everything the program printed.
        output: String,
    },
    /// The program breaks a decision, so the compiler would refuse it.
    Refused {
        /// The line the problem is on.
        line: usize,
        /// What is wrong, citing the decision.
        message: String,
    },
    /// The program uses something the design hasn't decided yet.
    NotDecided {
        /// The line the open question is on.
        line: usize,
        /// What is still open.
        message: String,
    },
    /// The program uses something decided that the oracle doesn't run yet.
    NotBuilt {
        /// The line it is on.
        line: usize,
        /// What is missing from the oracle.
        message: String,
    },
    /// A check site failed while the program ran.
    CheckFails {
        /// The line of the check site.
        line: usize,
        /// Which check failed, citing the decision.
        message: String,
        /// What the program printed before the check failed.
        output: String,
    },
    /// The oracle stopped on one of its own limits: too many steps or calls nested too deep. Not a judgement on the
    /// program.
    GaveUp {
        /// The line the oracle was on.
        line: usize,
        /// Which limit was reached.
        message: String,
        /// What the program printed before the oracle stopped.
        output: String,
    },
    /// The oracle contradicted itself: a bug in the oracle, never in the program.
    OracleFault {
        /// What went wrong inside the oracle.
        message: String,
    },
}

impl Outcome {
    /// The verdict without its explanation, as a test's `// expect:` line writes it: `runs`, `refused at line 4`,
    /// `not decided at line 4`, `not built at line 4`, `check fails at line 4`, `gives up at line 4` or
    /// `oracle fault`.
    #[must_use]
    pub fn verdict(&self) -> String {
        match self {
            Self::Runs { .. } => "runs".to_owned(),
            Self::Refused { line, .. } => format!("refused at line {line}"),
            Self::NotDecided { line, .. } => format!("not decided at line {line}"),
            Self::NotBuilt { line, .. } => format!("not built at line {line}"),
            Self::CheckFails { line, .. } => format!("check fails at line {line}"),
            Self::GaveUp { line, .. } => format!("gives up at line {line}"),
            Self::OracleFault { .. } => "oracle fault".to_owned(),
        }
    }

    /// The verdict with its explanation, for people.
    #[must_use]
    pub fn report(&self) -> String {
        match self {
            Self::Runs { .. } => self.verdict(),
            Self::Refused { message, .. }
            | Self::NotDecided { message, .. }
            | Self::NotBuilt { message, .. }
            | Self::CheckFails { message, .. }
            | Self::GaveUp { message, .. }
            | Self::OracleFault { message } => format!("{}: {message}", self.verdict()),
        }
    }

    /// What the program printed, if it got to run.
    #[must_use]
    pub fn output(&self) -> &str {
        match self {
            Self::Runs { output }
            | Self::CheckFails { output, .. }
            | Self::GaveUp { output, .. } => output,
            Self::Refused { .. }
            | Self::NotDecided { .. }
            | Self::NotBuilt { .. }
            | Self::OracleFault { .. } => "",
        }
    }
}

/// Which kind of problem was found before the program ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProblemKind {
    /// It breaks a decision.
    Refused,
    /// It needs a decision that hasn't been made.
    NotDecided,
    /// It needs a part of the oracle that isn't written yet.
    NotBuilt,
}

/// A problem found while reading or checking a program, before it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) kind: ProblemKind,
    pub(crate) line: usize,
    pub(crate) message: String,
}

impl Problem {
    /// The program breaks a decision.
    pub(crate) fn refused(line: usize, message: impl Into<String>) -> Self {
        Self {
            kind: ProblemKind::Refused,
            line,
            message: message.into(),
        }
    }

    /// The program needs a decision that hasn't been made.
    pub(crate) fn not_decided(line: usize, message: impl Into<String>) -> Self {
        Self {
            kind: ProblemKind::NotDecided,
            line,
            message: message.into(),
        }
    }

    /// The program needs a part of the oracle that isn't written yet.
    pub(crate) fn not_built(line: usize, message: impl Into<String>) -> Self {
        Self {
            kind: ProblemKind::NotBuilt,
            line,
            message: message.into(),
        }
    }
}

impl From<Problem> for Outcome {
    fn from(problem: Problem) -> Self {
        let Problem {
            kind,
            line,
            message,
        } = problem;
        match kind {
            ProblemKind::Refused => Self::Refused { line, message },
            ProblemKind::NotDecided => Self::NotDecided { line, message },
            ProblemKind::NotBuilt => Self::NotBuilt { line, message },
        }
    }
}
// end section oracle.outcome
