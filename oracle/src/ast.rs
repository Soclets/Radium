// section oracle.ast:
//! The program's tree, as the parser builds it.

use num_bigint::BigInt;

use crate::outcome::Place;

/// A whole program: its types, its functions and its `fn.main` (D101).
#[derive(Debug)]
pub(crate) struct Program {
    pub(crate) structs: Vec<StructDefinition>,
    pub(crate) unions: Vec<UnionDefinition>,
    pub(crate) functions: Vec<Function>,
    pub(crate) main: Function,
}

/// `struct name:` … `end struct name` (D58), optionally `struct.must_Give` (D84).
#[derive(Debug)]
pub(crate) struct StructDefinition {
    pub(crate) name: String,
    pub(crate) place: Place,
    /// `must_Give`: handed off with `Give` exactly once (D5, D84).
    pub(crate) must_give: bool,
    pub(crate) fields: Vec<Field>,
}

/// One field, `name: type;`, or `name.immut: type;` to lock it for good (D58).
#[derive(Debug)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) place: Place,
    pub(crate) locked: bool,
    pub(crate) type_spec: TypeSpec,
}

/// `union.tagged name:` … `end union.tagged name` (D59), optionally narrowed with `must_Give` (D84).
#[derive(Debug)]
pub(crate) struct UnionDefinition {
    pub(crate) name: String,
    pub(crate) place: Place,
    pub(crate) must_give: bool,
    pub(crate) cases: Vec<Case>,
}

/// One case: `ok;`, or `failed[code: int[…]];` carrying data (D59).
#[derive(Debug)]
pub(crate) struct Case {
    pub(crate) name: String,
    pub(crate) place: Place,
    pub(crate) fields: Vec<Field>,
}

/// A function definition: header, top of body, body (D50–D54).
#[derive(Debug)]
pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) place: Place,
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
    pub(crate) place: Place,
    pub(crate) changeability: Changeability,
    /// How the function holds the value (D91, D92).
    pub(crate) holding: Holding,
    pub(crate) type_spec: TypeSpec,
}

/// How a parameter holds its value (D91, D92).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Holding {
    /// Bare: lent, read-only; the caller keeps it.
    Lent,
    /// `Lent.mut`: may change it; the caller keeps it.
    LentMut,
    /// `Given`: the function owns it from then on.
    Given,
}

/// What a call does with a value it passes (D80, D84).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Handoff {
    Give,
    Copy,
    Lend,
    LendMut,
}

impl Handoff {
    /// How it is written.
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Give => "Give",
            Self::Copy => "Copy",
            Self::Lend => "Lend",
            Self::LendMut => "Lend.mut",
        }
    }
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
    pub(crate) place: Place,
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
    pub(crate) place: Place,
    pub(crate) parameter: String,
    pub(crate) value: Expression,
}

/// The types the oracle reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TypeSpec {
    /// `int[min: …, max: …]` (D14, D27).
    Integer(Bounds),
    /// `bool` (D60).
    Bool,
    /// A `struct` or `union.tagged` the program declares (D58, D59).
    Named(String),
    /// `array[element, length: N]` (D70).
    Array {
        element: Box<TypeSpec>,
        length: BigInt,
    },
}

impl TypeSpec {
    /// Which kind of value the type holds.
    pub(crate) fn kind(&self) -> ValueKind {
        match self {
            Self::Integer(_) => ValueKind::Integer,
            Self::Bool => ValueKind::Bool,
            Self::Named(name) => ValueKind::Named(name.clone()),
            Self::Array { .. } => ValueKind::Array,
        }
    }

    /// The integer bounds, for an integer type.
    pub(crate) fn bounds(&self) -> Option<&Bounds> {
        match self {
            Self::Integer(bounds) => Some(bounds),
            Self::Bool | Self::Named(_) | Self::Array { .. } => None,
        }
    }
}

/// An integer type's lower and upper bounds, both included (D14, D74). Any size: integers have no width cap (D104).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bounds {
    pub(crate) min: BigInt,
    pub(crate) max: BigInt,
}

/// The kinds of value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ValueKind {
    Integer,
    Bool,
    /// A `struct` or `union.tagged`, by name.
    Named(String),
    Array,
}

impl ValueKind {
    /// How error messages name the kind.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::Integer => "an integer".to_owned(),
            Self::Bool => "a `bool`".to_owned(),
            Self::Named(name) => format!("a `{name}`"),
            Self::Array => "an `array`".to_owned(),
        }
    }

    /// Whether values of this kind are always copied, so they take no ownership word (D92).
    pub(crate) fn is_copied(&self) -> bool {
        matches!(self, Self::Integer | Self::Bool)
    }
}

/// One statement or block, with the line it starts on.
#[derive(Debug)]
pub(crate) struct Statement {
    pub(crate) place: Place,
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
    /// `$x = value;` or `$x.field = value;` (D41, D58).
    Assign { target: Target, value: Expression },
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
    /// `match $x:` with one `case` per case of a `union.tagged` (D72).
    Match {
        subject: Expression,
        cases: Vec<MatchCase>,
    },
}

/// What an assignment changes: a value, or a field inside it, `$r.celsius` (D58).
#[derive(Debug)]
pub(crate) struct Target {
    pub(crate) name: String,
    pub(crate) fields: Vec<String>,
}

/// `case status.failed[code = $c]:` and its body (D72).
#[derive(Debug)]
pub(crate) struct MatchCase {
    pub(crate) place: Place,
    pub(crate) type_name: String,
    pub(crate) case_name: String,
    /// `code = $c`: a field of the case, and the name it is bound to.
    pub(crate) bindings: Vec<CaseBinding>,
    pub(crate) body: Vec<Statement>,
}

/// `field = $name` in a `case` (D72).
#[derive(Debug)]
pub(crate) struct CaseBinding {
    pub(crate) place: Place,
    pub(crate) field: String,
    pub(crate) name: String,
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
    pub(crate) place: Place,
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
    pub(crate) place: Place,
    pub(crate) label: Option<String>,
    /// `Give`, `Copy`, `Lend` or `Lend.mut` before the value (D80, D84).
    pub(crate) handoff: Option<Handoff>,
    pub(crate) value: Expression,
}

/// An expression, with the line it starts on.
#[derive(Debug)]
pub(crate) struct Expression {
    pub(crate) place: Place,
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
    /// Building a value: `reading[time = 3600]`, `status.ok`, `status.failed[code = 3]` (D58, D59).
    Build {
        type_name: String,
        case: Option<String>,
        fields: Vec<FieldValue>,
    },
    /// Reading a field: `$r.celsius` (D26, D58).
    Field {
        base: Box<Expression>,
        name: String,
    },
    Negate(Box<Expression>),
    Not(Box<Expression>),
    Binary {
        operator: Operator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
}

/// `field = value` inside a build (D50, D58).
#[derive(Debug)]
pub(crate) struct FieldValue {
    pub(crate) place: Place,
    pub(crate) name: String,
    pub(crate) value: Expression,
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
