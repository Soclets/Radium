// section oracle.ast:
//! The program's tree, as the parser builds it.

use num_bigint::BigInt;

/// A whole program: its functions and its `fn.main` (D101).
#[derive(Debug)]
pub(crate) struct Program {
    pub(crate) functions: Vec<Function>,
    pub(crate) main: Function,
}

/// A function definition: header, top of body, body (D50–D54).
#[derive(Debug)]
pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) line: usize,
    /// `fn.pure` (D51).
    pub(crate) pure: bool,
    /// `fn.main`: where the program starts; nothing may call it (D101).
    pub(crate) is_main: bool,
    pub(crate) parameters: Vec<Parameter>,
    /// The type after `->` (D52, D98); `None` for `-> void` (D99).
    pub(crate) result: Option<TypeSpec>,
    pub(crate) labels: Option<Labels>,
    pub(crate) defaults: Vec<ParameterDefault>,
    pub(crate) requires: Vec<Expression>,
    pub(crate) ensures: Vec<Expression>,
    pub(crate) body: Vec<Statement>,
}

/// One parameter, `$name: type` (D31, D50).
#[derive(Debug)]
pub(crate) struct Parameter {
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) changeability: Changeability,
    pub(crate) type_spec: TypeSpec,
}

/// Whether a parameter may be reassigned inside its function (D31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Changeability {
    /// Bare: `var.immut`, never reassigned.
    Fixed,
    /// `var`: may be reassigned.
    Free,
    /// `var.mut`: must be reassigned somewhere (D30).
    MustChange,
}

/// A `labels` line (D54).
#[derive(Debug)]
pub(crate) struct Labels {
    pub(crate) line: usize,
    /// `labels.any_order`.
    pub(crate) any_order: bool,
    pub(crate) entries: Vec<LabelEntry>,
}

/// `$parameter as label`.
#[derive(Debug)]
pub(crate) struct LabelEntry {
    pub(crate) parameter: String,
    pub(crate) label: String,
}

/// One entry of a `defaults` line, `$parameter = value` (D54).
#[derive(Debug)]
pub(crate) struct ParameterDefault {
    pub(crate) line: usize,
    pub(crate) parameter: String,
    pub(crate) value: Expression,
}

/// The types of the first slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypeSpec {
    /// `int[min: …, max: …]` (D14, D27).
    Integer(Bounds),
    /// `bool` (D60).
    Bool,
}

impl TypeSpec {
    /// Which kind of value the type holds.
    pub(crate) fn kind(&self) -> ValueKind {
        match self {
            Self::Integer(_) => ValueKind::Integer,
            Self::Bool => ValueKind::Bool,
        }
    }

    /// The integer bounds, for an integer type.
    pub(crate) fn bounds(&self) -> Option<&Bounds> {
        match self {
            Self::Integer(bounds) => Some(bounds),
            Self::Bool => None,
        }
    }
}

/// An integer type's lower and upper bounds, both included (D14, D74). Any size: integers have no width cap (D104).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bounds {
    pub(crate) min: BigInt,
    pub(crate) max: BigInt,
}

/// The kinds of value in the first slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueKind {
    Integer,
    Bool,
}

impl ValueKind {
    /// How error messages name the kind.
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::Integer => "an integer",
            Self::Bool => "a `bool`",
        }
    }
}

/// One statement or block, with the line it starts on.
#[derive(Debug)]
pub(crate) struct Statement {
    pub(crate) line: usize,
    pub(crate) kind: StatementKind,
}

/// The statements and blocks of the first slice.
#[derive(Debug)]
pub(crate) enum StatementKind {
    /// `var $x: type = value;` and its narrowings, or `const` (D30, D41); `type_spec` is `None` for `: infer` (D108).
    Declare {
        holder: Holder,
        name: String,
        type_spec: Option<TypeSpec>,
        value: Expression,
    },
    /// `$x = value;` (D41).
    Assign { name: String, value: Expression },
    /// A call run for what it does.
    Call(Call),
    /// `if` / `else if` / `else` (D71).
    If {
        branches: Vec<Branch>,
        else_body: Option<Vec<Statement>>,
    },
    /// `for $x in range[…]:` (D73, D74), optionally named (D77).
    For {
        loop_name: Option<String>,
        variable: String,
        range: LoopRange,
        body: Vec<Statement>,
    },
    /// `while …:` with an optional `decreases` (D75).
    While {
        condition: Expression,
        decreases: Option<Expression>,
        body: Vec<Statement>,
    },
    /// `forever:` (D76), optionally named.
    Forever {
        loop_name: Option<String>,
        body: Vec<Statement>,
    },
    /// `break;` or `break name;` (D77).
    Break { loop_name: Option<String> },
    /// `return value;` (D95), or `return void;` with `None` (D100).
    Return { value: Option<Expression> },
}

/// How a value is declared (D30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Holder {
    Var,
    VarImmut,
    VarMut,
    Const,
}

/// One `if` or `else if` branch.
#[derive(Debug)]
pub(crate) struct Branch {
    pub(crate) condition: Expression,
    pub(crate) body: Vec<Statement>,
}

/// `range[from: …, to: … | before: …, step_by: …]` (D73, D74).
#[derive(Debug)]
pub(crate) struct LoopRange {
    pub(crate) from: Expression,
    pub(crate) end: RangeEnd,
    pub(crate) step_by: Option<Expression>,
}

/// Where a range stops.
#[derive(Debug)]
pub(crate) enum RangeEnd {
    /// `to:` includes the end.
    To(Expression),
    /// `before:` stops before it.
    Before(Expression),
}

/// A call: `( )` runs something (D50).
#[derive(Debug)]
pub(crate) struct Call {
    pub(crate) line: usize,
    pub(crate) target: CallTarget,
    pub(crate) arguments: Vec<Argument>,
}

/// What a call runs.
#[derive(Debug)]
pub(crate) enum CallTarget {
    /// `std::print` (D26, D57).
    Print,
    /// The writer's own function; `marked` when written `my.name` (D43).
    Own { name: String, marked: bool },
}

/// One argument, labelled or not (D54).
#[derive(Debug)]
pub(crate) struct Argument {
    pub(crate) line: usize,
    pub(crate) label: Option<String>,
    pub(crate) value: Expression,
}

/// An expression, with the line it starts on.
#[derive(Debug)]
pub(crate) struct Expression {
    pub(crate) line: usize,
    pub(crate) kind: ExpressionKind,
}

/// The expressions of the first slice.
#[derive(Debug)]
pub(crate) enum ExpressionKind {
    Integer(BigInt),
    Bool(bool),
    /// Text, allowed only as a `std::print` item while text types are open.
    Text(String),
    /// `newline` (D94).
    Newline,
    /// A value's `$name` (D31).
    Value(String),
    /// `old[$parameter]` (D53).
    Old(String),
    /// `result` in an `ensures`: the value the function gives back (D53).
    Result,
    /// A call to a function with a result, used as a value (D95).
    Call(Call),
    Negate(Box<Expression>),
    Not(Box<Expression>),
    Binary {
        operator: Operator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
}

/// The operators of the first slice (D34, D37, D38, D40).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide(Rounding),
    Remainder(RemainderRule),
    Less,
    Greater,
    AtMost,
    AtLeast,
    Equal,
    NotEqual,
    And,
    Or,
}

/// `div` and its narrowings (D37).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rounding {
    /// Bare `div`: must leave no remainder.
    Exact,
    Floor,
    Trunc,
    Ceil,
}

/// `mod` and its narrowings (D38).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemainderRule {
    /// Bare `mod`: both numbers must be non-negative.
    NonNegative,
    Floor,
    Trunc,
    Euclid,
}
// end section oracle.ast
