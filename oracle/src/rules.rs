// section oracle.rules:
//! Rules the checker and the interpreter share, so both read a decision the same way.

/// Words Radium itself uses. A writer's function with one of these names is called `my.name` (D43).
pub(crate) const RADIUM_WORDS: &[&str] = &[
    "and",
    "at_least",
    "at_most",
    "bool",
    "break",
    "case",
    "comment",
    "const",
    "decreases",
    "defaults",
    "div",
    "else",
    "end",
    "ensures",
    "equal",
    "false",
    "fn",
    "for",
    "forever",
    "if",
    "in",
    "int",
    "labels",
    "match",
    "mod",
    "my",
    "newline",
    "not",
    "not_equal",
    "old",
    "or",
    "range",
    "requires",
    "result",
    "return",
    "std",
    "struct",
    "true",
    "union",
    "var",
    "void",
    "while",
    "x",
];

/// Whether `name` is one of Radium's own words.
pub(crate) fn is_radium_word(name: &str) -> bool {
    RADIUM_WORDS.contains(&name)
}

/// Whether a loop from `from` towards `end` in steps of `step` can ever reach its end (D74). A step of zero never
/// moves; a step pointing away from the end never arrives.
pub(crate) fn range_can_end(from: i128, end: i128, step: i128) -> bool {
    // trace D74: `from: 5, to: 1` with no negative step is refused.
    match step.signum() {
        1 => from <= end,
        -1 => from >= end,
        _ => false,
    }
}
// end section oracle.rules
