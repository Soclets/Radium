// section oracle.outcome:
//! What judging a program can end in, and the messages that explain it (D116–D119, D122).

use unicode_segmentation::UnicodeSegmentation;

/// Where something is in the source (D117): its line, its column twice, and how wide it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// The line, counting from 1.
    pub line: usize,
    /// The column for tools: UTF-8 bytes from the start of the line, counting from 1.
    pub byte_column: usize,
    /// The column for people: grapheme clusters from the start of the line, counting from 1.
    pub column: usize,
    /// How many grapheme clusters the part in question covers, at least 1.
    pub width: usize,
}

impl Place {
    /// The whole span from this place to the end of `end`, when both are on one line; otherwise just this place.
    pub(crate) fn to(self, end: Self) -> Self {
        if end.line != self.line || end.column < self.column {
            return self;
        }
        let width = end
            .column
            .saturating_sub(self.column)
            .saturating_add(end.width);
        Self { width, ..self }
    }
}

/// One `because:` link: an earlier step that led to the problem, and what happened there (D122).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Where it happened.
    pub place: Place,
    /// One short fact about what happened there.
    pub fact: String,
}

/// The kind of a message, its first word (D119). The last three are the oracle's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The code breaks a rule.
    Refused,
    /// A check failed while the program ran (D114).
    CheckFailed,
    /// The program needs a decision that hasn't been made.
    NotDecided,
    /// The program needs a part of the oracle that isn't written yet.
    NotBuilt,
    /// The oracle reached one of its own limits.
    GaveUp,
}

impl Kind {
    /// The word that starts a message's first line.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::CheckFailed => "check failed",
            Self::NotDecided => "not decided",
            Self::NotBuilt => "not built",
            Self::GaveUp => "gave up",
        }
    }
}

/// A message about one problem, with only what's certain (D116).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Its kind.
    pub kind: Kind,
    /// Where the problem is.
    pub place: Place,
    /// What is wrong.
    pub what: String,
    /// The one fix, when there is exactly one; shown in place of `what` (D116, D118).
    pub fix: Option<String>,
    /// Why: the rule, with its decision number.
    pub why: String,
    /// When: the values that break it, where they are known.
    pub when: Option<String>,
    /// Earlier steps that led to it, in the order they happened (D122).
    pub because: Vec<Link>,
}

impl Message {
    /// The message in one line, for test reports.
    #[must_use]
    pub fn summary(&self) -> String {
        match &self.fix {
            Some(fix) => format!("{}; fix: {fix}; why: {}", self.what, self.why),
            None => format!("{}; why: {}", self.what, self.why),
        }
    }

    /// The message laid out as D117–D119 and D122 say, quoting lines from `source`, the file named `file`.
    #[must_use]
    pub fn render(&self, file: &str, source: &str) -> String {
        let lines: Vec<&str> = source.lines().collect();
        let widest = self
            .because
            .iter()
            .map(|link| link.place.line)
            .chain([self.place.line])
            .max()
            .unwrap_or(1)
            .to_string()
            .len();
        let mut parts = vec![
            format!("{}: {}\n", self.kind.word(), place_text(file, self.place)),
            quote(&lines, self.place, widest),
        ];
        match &self.fix {
            Some(fix) => parts.push(format!(" fix:  {fix}\n")),
            None => parts.push(format!(" what: {}\n", self.what)),
        }
        parts.push(format!(" why:  {}\n", self.why));
        if let Some(when) = &self.when {
            parts.push(format!(" when: {when}\n"));
        }
        for link in &self.because {
            parts.push(format!(" because: {}\n", place_text(file, link.place)));
            parts.push(quote(&lines, link.place, widest));
            // The fact starts where the quoted code does.
            parts.push(format!(" {:widest$}   {}\n", "", link.fact));
        }
        parts.concat()
    }
}

/// `file:line:byte_column, line N column M` (D117).
fn place_text(file: &str, place: Place) -> String {
    format!(
        "{file}:{}:{}, line {} column {}",
        place.line, place.byte_column, place.line, place.column
    )
}

/// The quoted line with its number in the margin, and a `^` pointer under the part (D118).
fn quote(lines: &[&str], place: Place, widest: usize) -> String {
    let Some(text) = place.line.checked_sub(1).and_then(|index| lines.get(index)) else {
        return String::new();
    };
    // A tab is one grapheme cluster, so it is shown as one space to keep the pointer under its part.
    let shown = text.replace('\t', " ");
    let before: String = shown
        .graphemes(true)
        .take(place.column.saturating_sub(1))
        .map(|_| " ")
        .collect();
    let pointer = "^".repeat(place.width.max(1));
    format!(
        " {:>widest$} | {shown}\n {:widest$} | {before}{pointer}\n",
        place.line, ""
    )
}

/// How the oracle judged a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The program ran to the end of `fn.main`.
    Runs {
        /// Everything the program printed.
        output: String,
    },
    /// The program was stopped before or while it ran, with the message saying why.
    Stopped {
        /// The message.
        message: Message,
        /// What the program printed before it stopped (empty when it never ran).
        output: String,
    },
    /// The oracle contradicted itself: a bug in the oracle, never in the program.
    OracleFault {
        /// What went wrong inside the oracle.
        message: String,
    },
}

impl Outcome {
    /// The verdict without its explanation, as a test's `// expect:` line writes it (D103): `runs`, `refused at
    /// line 4`, `not decided at line 4`, `not built at line 4`, `check fails at line 4`, `gives up at line 4` or
    /// `oracle fault`.
    #[must_use]
    pub fn verdict(&self) -> String {
        match self {
            Self::Runs { .. } => "runs".to_owned(),
            Self::Stopped { message, .. } => {
                let line = message.place.line;
                match message.kind {
                    Kind::Refused => format!("refused at line {line}"),
                    Kind::NotDecided => format!("not decided at line {line}"),
                    Kind::NotBuilt => format!("not built at line {line}"),
                    Kind::CheckFailed => format!("check fails at line {line}"),
                    Kind::GaveUp => format!("gives up at line {line}"),
                }
            }
            Self::OracleFault { .. } => "oracle fault".to_owned(),
        }
    }

    /// The verdict with its explanation on one line, for test reports.
    #[must_use]
    pub fn report(&self) -> String {
        match self {
            Self::Runs { .. } => self.verdict(),
            Self::Stopped { message, .. } => format!("{}: {}", self.verdict(), message.summary()),
            Self::OracleFault { message } => format!("oracle fault: {message}"),
        }
    }

    /// The full message, laid out for people (D117–D119, D122); empty when the program ran.
    #[must_use]
    pub fn render(&self, file: &str, source: &str) -> String {
        match self {
            Self::Runs { .. } => String::new(),
            Self::Stopped { message, .. } => message.render(file, source),
            Self::OracleFault { message } => format!("oracle fault: {message}\n"),
        }
    }

    /// What the program printed, if it got to run.
    #[must_use]
    pub fn output(&self) -> &str {
        match self {
            Self::Runs { output } | Self::Stopped { output, .. } => output,
            Self::OracleFault { .. } => "",
        }
    }
}

/// A problem found while reading, checking or running a program: a boxed `Message`, so that results stay small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Problem(Box<Message>);

impl Problem {
    fn new(kind: Kind, place: Place, what: impl Into<String>, why: impl Into<String>) -> Self {
        Self(Box::new(Message {
            kind,
            place,
            what: what.into(),
            fix: None,
            why: why.into(),
            when: None,
            because: Vec::new(),
        }))
    }

    /// The program breaks a rule.
    pub(crate) fn refused(place: Place, what: impl Into<String>, why: impl Into<String>) -> Self {
        Self::new(Kind::Refused, place, what, why)
    }

    /// A check failed while the program ran.
    pub(crate) fn check_failed(
        place: Place,
        what: impl Into<String>,
        why: impl Into<String>,
    ) -> Self {
        Self::new(Kind::CheckFailed, place, what, why)
    }

    /// The program needs a decision that hasn't been made.
    pub(crate) fn not_decided(place: Place, what: impl Into<String>) -> Self {
        Self::new(
            Kind::NotDecided,
            place,
            what,
            "the design hasn't decided this yet, so the oracle won't guess",
        )
    }

    /// The program needs a part of the oracle that isn't written yet.
    pub(crate) fn not_built(place: Place, what: impl Into<String>) -> Self {
        Self::new(
            Kind::NotBuilt,
            place,
            what,
            "this is decided, but the oracle doesn't run it yet",
        )
    }

    /// The oracle reached one of its own limits.
    pub(crate) fn gave_up(place: Place, what: impl Into<String>) -> Self {
        Self::new(
            Kind::GaveUp,
            place,
            what,
            "this is the oracle's own limit, not a judgement on the program",
        )
    }

    /// Adds the one fix (D116).
    pub(crate) fn fix(mut self, code: impl Into<String>) -> Self {
        self.0.fix = Some(code.into());
        self
    }

    /// Adds the values that break it.
    pub(crate) fn when(mut self, values: impl Into<String>) -> Self {
        self.0.when = Some(values.into());
        self
    }

    /// Adds a `because:` link (D122).
    pub(crate) fn because(mut self, place: Place, fact: impl Into<String>) -> Self {
        self.0.because.push(Link {
            place,
            fact: fact.into(),
        });
        self
    }

    /// What is wrong.
    pub(crate) fn what(&self) -> &str {
        &self.0.what
    }

    /// The message itself.
    pub(crate) fn into_message(self) -> Message {
        *self.0
    }
}
// end section oracle.outcome
