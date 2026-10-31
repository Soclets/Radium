// section oracle.checker:
//! Refuses what the compiler would refuse, before anything runs: names and kinds of values, changes to values that
//! may not change (D30, D31, D58), conditions that aren't `bool` (D60), `break` outside a loop (D77), `return` and
//! `result` (D53, D95), building and reading structs and unions and `match` (D58, D59, D72), ownership (D80, D84,
//! D91, D92), and calls: labels, defaults, `my.` and `fn.pure` (D43, D51, D54).

use std::collections::HashMap;

use num_bigint::BigInt;

use crate::ast::{
    Argument, Call, CallTarget, Case, Changeability, Expression, ExpressionKind, FieldValue,
    Function, Handoff, Holder, Holding, LoopRange, MatchCase, Operator, Parameter, Program,
    RangeEnd, Statement, StatementKind, Target, TypeSpec, ValueKind,
};
use crate::binding::{Binding, bind_arguments};
use crate::outcome::{Place, Problem};
use crate::rules::{is_radium_word, range_can_end};
use crate::types::Types;

/// Checks a whole program.
pub(crate) fn check(program: &Program) -> Result<(), Problem> {
    let types = Types::new(program)?;
    let mut functions = HashMap::new();
    for function in &program.functions {
        if functions.insert(function.name.as_str(), function).is_some() {
            return Err(Problem::not_decided(
                function.place,
                format!("two functions named `{}`", function.name),
            ));
        }
    }
    let mut checker = Checker {
        types,
        functions,
        scopes: Vec::new(),
        loops: Vec::new(),
        function: None,
    };
    for function in &program.functions {
        checker.function_definition(function)?;
    }
    let main = &program.main;
    if !main.parameters.is_empty() {
        return Err(Problem::not_decided(
            main.place,
            "parameters for `fn.main`, such as command-line arguments (D101)",
        ));
    }
    if main.result.is_some() {
        return Err(Problem::not_decided(
            main.place,
            "what `fn.main` may give back besides `void`, such as an exit status (D101)",
        ));
    }
    checker.function_definition(main)
}

/// How a name was introduced, which decides whether it may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Var,
    VarImmut,
    VarMut,
    Const,
    Parameter {
        changeability: Changeability,
        holding: Holding,
    },
    LoopValue,
    /// A value a `case` pulled out of a union (D72).
    CaseValue,
}

impl Role {
    /// Whether the value, and the fields inside it, may change (D30, D31, D58, D91).
    fn may_change(self) -> bool {
        match self {
            Self::Var | Self::VarMut => true,
            Self::Parameter {
                changeability,
                holding,
            } => holding == Holding::LentMut || changeability != Changeability::Fixed,
            Self::VarImmut | Self::Const | Self::LoopValue | Self::CaseValue => false,
        }
    }
}

/// Whether a value has been handed off with `Give` (D80).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Given {
    /// On some paths only.
    Maybe(Place),
    /// On every path.
    Yes(Place),
}

impl Given {
    fn place(self) -> Place {
        match self {
            Self::Maybe(place) | Self::Yes(place) => place,
        }
    }
}

#[derive(Debug, Clone)]
struct Known {
    role: Role,
    kind: ValueKind,
    /// Where it was declared, for `because:` links (D122).
    place: Place,
    changed: bool,
    given: Option<Given>,
}

/// Where an expression sits. Contracts may call only `fn.pure` functions (D51); `old[…]` and `result` belong to an
/// `ensures` (D53).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    Plain,
    /// A `requires` or a `decreases`.
    Contract,
    Ensures,
}

/// A loop being checked: its name, and how many scopes were open when it started.
struct Loop {
    name: Option<String>,
    depth: usize,
}

type Scopes = Vec<HashMap<String, Known>>;

struct Checker<'program> {
    types: Types<'program>,
    functions: HashMap<&'program str, &'program Function>,
    scopes: Scopes,
    loops: Vec<Loop>,
    function: Option<&'program Function>,
}

// Functions, blocks and the paths through them.
impl<'program> Checker<'program> {
    fn function_definition(&mut self, function: &'program Function) -> Result<(), Problem> {
        self.function = Some(function);
        self.scopes.push(HashMap::new());
        for parameter in &function.parameters {
            self.parameter(parameter)?;
        }
        if let Some(result) = &function.result {
            self.types.check_type(result, function.place)?;
            if self.types.is_must_give(&result.kind()) {
                return Err(Problem::not_decided(
                    function.place,
                    "giving back a `must_Give` value from a function (D84)",
                ));
            }
        }
        labels_and_defaults(function)?;
        for condition in &function.requires {
            self.condition(condition, Context::Contract, "a `requires`")?;
        }
        for condition in &function.ensures {
            self.condition(condition, Context::Ensures, "an `ensures`")?;
        }
        self.block(&function.body)?;
        if !always_returns(&function.body) {
            return Err(Problem::refused(
                function.place,
                format!("`{}` can reach its end without `return`", function.name),
                "every path through a function ends in `return`, which for `-> void` is `return void;` (D96, D102)",
            ));
        }
        self.pop_scope()
    }

    fn parameter(&mut self, parameter: &Parameter) -> Result<(), Problem> {
        self.types
            .check_type(&parameter.type_spec, parameter.place)?;
        let kind = parameter.type_spec.kind();
        if kind.is_copied() && parameter.holding != Holding::Lent {
            let word = if parameter.holding == Holding::Given {
                "Given"
            } else {
                "Lent.mut"
            };
            return Err(Problem::refused(
                parameter.place,
                format!("`{word}` on a value that is always copied"),
                "integers and `bool` are always copied, so they take no ownership word (D92)",
            ));
        }
        if parameter.holding == Holding::Given && self.types.is_must_give(&kind) {
            return Err(Problem::not_decided(
                parameter.place,
                "how the duty to give a `must_Give` value ends inside the function that receives it (D84)",
            ));
        }
        if let Some(first) = self.lookup(&parameter.name) {
            return Err(Problem::refused(
                parameter.place,
                format!("a second parameter named `${}`", parameter.name),
                "a function's parameters have different names (D31)",
            )
            .because(first.place, "the first one is here"));
        }
        self.declare(
            &parameter.name,
            Role::Parameter {
                changeability: parameter.changeability,
                holding: parameter.holding,
            },
            kind,
            parameter.place,
        );
        Ok(())
    }

    fn block(&mut self, statements: &'program [Statement]) -> Result<(), Problem> {
        self.scopes.push(HashMap::new());
        for statement in statements {
            self.statement(statement)?;
        }
        self.pop_scope()
    }

    /// Checks one branch from `before`, giving back the state after it, or `None` when the branch never reaches its
    /// end (it returns or breaks out).
    fn branch(
        &mut self,
        before: &Scopes,
        body: &'program [Statement],
    ) -> Result<Option<Scopes>, Problem> {
        self.scopes.clone_from(before);
        self.block(body)?;
        if always_returns(body) || ends_in_break(body) {
            Ok(None)
        } else {
            Ok(Some(self.scopes.clone()))
        }
    }

    /// Joins the states after several paths: a value is given on every path, on some, or on none.
    fn merge(&mut self, before: Scopes, outcomes: &[Scopes]) {
        self.scopes = before;
        for (depth, scope) in self.scopes.iter_mut().enumerate() {
            for (name, known) in scope.iter_mut() {
                let states: Vec<Option<Given>> = outcomes
                    .iter()
                    .map(|outcome| {
                        outcome
                            .get(depth)
                            .and_then(|scope| scope.get(name))
                            .and_then(|known| known.given)
                    })
                    .collect();
                let first = states.iter().flatten().next().map(|given| given.place());
                known.given = match first {
                    None => None,
                    Some(place)
                        if states
                            .iter()
                            .all(|state| matches!(state, Some(Given::Yes(_)))) =>
                    {
                        Some(Given::Yes(place))
                    }
                    Some(place) => Some(Given::Maybe(place)),
                };
                known.changed |= outcomes.iter().any(|outcome| {
                    outcome
                        .get(depth)
                        .and_then(|scope| scope.get(name))
                        .is_some_and(|known| known.changed)
                });
            }
        }
    }

    /// Leaves a scope, refusing any `var.mut` that nothing changed (D30) and any `must_Give` value not handed off
    /// on every path (D84).
    fn pop_scope(&mut self) -> Result<(), Problem> {
        let Some(scope) = self.scopes.pop() else {
            return Ok(());
        };
        let mut known: Vec<(&String, &Known)> = scope.iter().collect();
        known.sort_by_key(|(_, known)| (known.place.line, known.place.column));
        for (name, known) in &known {
            let must_change = matches!(
                known.role,
                Role::VarMut
                    | Role::Parameter {
                        changeability: Changeability::MustChange,
                        ..
                    }
            );
            if must_change && !known.changed {
                return Err(Problem::refused(
                    known.place,
                    format!("nothing changes `${name}`"),
                    "a `var.mut` must change somewhere (D30)",
                ));
            }
        }
        for (name, known) in &known {
            self.refuse_ungiven(name, known, "it goes out of scope")?;
        }
        Ok(())
    }

    /// Refuses leaving a `must_Give` value behind without handing it off (D84).
    fn refuse_ungiven(&self, name: &str, known: &Known, leaving: &str) -> Result<(), Problem> {
        let holds_duty = matches!(known.role, Role::Var | Role::VarImmut | Role::VarMut);
        if !holds_duty || !self.types.is_must_give(&known.kind) {
            return Ok(());
        }
        match known.given {
            Some(Given::Yes(_)) => Ok(()),
            Some(Given::Maybe(place)) => Err(Problem::refused(
                known.place,
                format!("`${name}` isn't handed off on every path before {leaving}"),
                "a `must_Give` value is handed off with `Give` exactly once (D5, D84)",
            )
            .because(place, "it is handed off here, on one path only")),
            None => Err(Problem::refused(
                known.place,
                format!("`${name}` is never handed off before {leaving}"),
                "a `must_Give` value is handed off with `Give` exactly once (D5, D84)",
            )),
        }
    }

    fn statement(&mut self, statement: &'program Statement) -> Result<(), Problem> {
        let place = statement.place;
        match &statement.kind {
            StatementKind::Declare {
                holder,
                name,
                type_spec,
                value,
            } => self.declaration(place, *holder, name, type_spec.as_ref(), value),
            StatementKind::Assign { target, value } => self.assignment(place, target, value),
            StatementKind::Call(call) => {
                if self.call(call, Context::Plain)?.is_some() {
                    return Err(Problem::refused(
                        call.place,
                        "this call's result is thrown away",
                        "a result can't be ignored (D97)",
                    ));
                }
                Ok(())
            }
            StatementKind::If {
                branches,
                else_body,
            } => {
                let before = self.scopes.clone();
                let mut outcomes = Vec::new();
                for branch in branches {
                    self.scopes.clone_from(&before);
                    self.condition(&branch.condition, Context::Plain, "an `if`")?;
                    outcomes.extend(self.branch(&before, &branch.body)?);
                }
                match else_body {
                    Some(body) => outcomes.extend(self.branch(&before, body)?),
                    None => outcomes.push(before.clone()),
                }
                self.merge(before, &outcomes);
                Ok(())
            }
            StatementKind::For {
                loop_name,
                variable,
                range,
                body,
            } => self.for_loop(place, loop_name.as_ref(), variable, range, body),
            StatementKind::While {
                condition,
                decreases,
                body,
            } => {
                self.condition(condition, Context::Plain, "a `while`")?;
                if let Some(measure) = decreases {
                    self.integer_in(measure, Context::Contract, "a `decreases`")?;
                }
                self.loop_body(None, body)
            }
            StatementKind::Forever { loop_name, body } => self.loop_body(loop_name.as_ref(), body),
            StatementKind::Break { loop_name } => self.break_statement(place, loop_name.as_ref()),
            StatementKind::Return { value } => self.return_statement(place, value.as_ref()),
            StatementKind::Match { subject, cases } => self.match_statement(subject, cases),
        }
    }

    fn loop_body(
        &mut self,
        loop_name: Option<&String>,
        body: &'program [Statement],
    ) -> Result<(), Problem> {
        if let Some(name) = loop_name
            && self
                .loops
                .iter()
                .any(|other| other.name.as_ref() == Some(name))
        {
            return Err(Problem::not_decided(
                body.first()
                    .map_or(self.function_place(), |statement| statement.place),
                format!("a loop named `{name}` inside another loop named `{name}`"),
            ));
        }
        let before = self.scopes.clone();
        self.loops.push(Loop {
            name: loop_name.cloned(),
            depth: self.scopes.len(),
        });
        let result = self.block(body);
        self.loops.pop();
        result?;
        // A value from outside the loop handed off inside it would be handed off again on the next trip.
        for (depth, scope) in self.scopes.iter().enumerate() {
            for (name, known) in scope {
                let was_given = before
                    .get(depth)
                    .and_then(|scope| scope.get(name))
                    .and_then(|known| known.given);
                if let (None, Some(given)) = (was_given, known.given) {
                    return Err(Problem::refused(
                        given.place(),
                        format!("`${name}` is handed off inside a loop"),
                        "a value is handed off with `Give` once; a later trip would hand it off again (D80)",
                    )
                    .because(known.place, format!("`${name}` is declared outside the loop")));
                }
            }
        }
        let after = self.scopes.clone();
        self.merge(before.clone(), &[before, after]);
        Ok(())
    }

    fn break_statement(&self, place: Place, loop_name: Option<&String>) -> Result<(), Problem> {
        let target = match loop_name {
            None => self.loops.last(),
            Some(name) => self
                .loops
                .iter()
                .rev()
                .find(|other| other.name.as_ref() == Some(name)),
        };
        let Some(target) = target else {
            if self.loops.is_empty() {
                return Err(Problem::refused(
                    place,
                    "`break` with no loop around it",
                    "`break` stops the loop it is in (D77)",
                ));
            }
            return Err(Problem::refused(
                place,
                format!(
                    "there is no loop named `{}` around this `break`",
                    loop_name.map_or("", String::as_str)
                ),
                "`break name;` stops the named loop it is in (D77)",
            ));
        };
        for scope in self.scopes.iter().skip(target.depth) {
            for (name, known) in scope {
                self.refuse_ungiven(name, known, "the `break` leaves it")?;
            }
        }
        Ok(())
    }

    fn return_statement(
        &mut self,
        place: Place,
        value: Option<&Expression>,
    ) -> Result<(), Problem> {
        let Some(function) = self.function else {
            return Err(Problem::refused(
                place,
                "`return` outside a function",
                "`return` ends a function (D95)",
            ));
        };
        let header = (
            function.place,
            format!("`{}`'s result is set here", function.name),
        );
        match (function.result.as_ref(), value) {
            (None, None) => {}
            (None, Some(_)) => {
                return Err(Problem::refused(
                    place,
                    format!("`{}` gives nothing back", function.name),
                    "a `-> void` function returns with `return void;` (D99, D100)",
                )
                .fix("`return void;`")
                .because(header.0, header.1));
            }
            (Some(result), None) => {
                return Err(Problem::refused(
                    place,
                    format!(
                        "`{}` gives back {}",
                        function.name,
                        result.kind().describe()
                    ),
                    "`return void;` is only for `-> void` functions (D100)",
                )
                .because(header.0, header.1));
            }
            (Some(result), Some(value)) => {
                let kind = self.expression(value, Context::Plain)?;
                if kind != result.kind() {
                    return Err(Problem::refused(
                        value.place,
                        format!(
                            "this is {}, but `{}` gives back {}",
                            kind.describe(),
                            function.name,
                            result.kind().describe()
                        ),
                        "a `return` gives back a value of the function's result type (D52, D95)",
                    )
                    .because(header.0, header.1));
                }
            }
        }
        for scope in &self.scopes {
            for (name, known) in scope {
                self.refuse_ungiven(name, known, "the `return`")?;
            }
        }
        Ok(())
    }
}

// Values: declaring, changing, and the loops that declare them.
impl<'program> Checker<'program> {
    fn declaration(
        &mut self,
        place: Place,
        holder: Holder,
        name: &str,
        type_spec: Option<&TypeSpec>,
        value: &Expression,
    ) -> Result<(), Problem> {
        let value_kind = self.expression(value, Context::Plain)?;
        if let Some(type_spec) = type_spec {
            self.types.check_type(type_spec, place)?;
            if type_spec.kind() != value_kind {
                return Err(Problem::refused(
                    value.place,
                    format!(
                        "this is {}, but `${name}` holds {}",
                        value_kind.describe(),
                        type_spec.kind().describe()
                    ),
                    "a value holds only values of its own type (D30)",
                ));
            }
        }
        if holder == Holder::Const && !value_kind.is_copied() {
            return Err(Problem::not_decided(
                place,
                "a `const` struct, union or array",
            ));
        }
        if holder == Holder::Const && !self.is_constant(value) {
            return Err(Problem::refused(
                value.place,
                "this value isn't known when compiling",
                "a `const` is known when compiling, so its value uses only numbers, `true`, `false` and other `const`s (D30)",
            ));
        }
        if let Some(first) = self.lookup(name) {
            return Err(Problem::not_decided(
                place,
                format!("declaring `${name}` again while the first is still visible (shadowing)"),
            )
            .because(first.place, format!("the first `${name}` is here")));
        }
        let role = match holder {
            Holder::Var => Role::Var,
            Holder::VarImmut => Role::VarImmut,
            Holder::VarMut => Role::VarMut,
            Holder::Const => Role::Const,
        };
        self.declare(name, role, value_kind, place);
        Ok(())
    }

    fn assignment(
        &mut self,
        place: Place,
        target: &Target,
        value: &Expression,
    ) -> Result<(), Problem> {
        let value_kind = self.expression(value, Context::Plain)?;
        let name = &target.name;
        let Some(known) = self.lookup(name).cloned() else {
            return Err(undeclared(place, name));
        };
        refuse_given(place, name, &known)?;
        let what = if target.fields.is_empty() {
            format!("`${name}` can't change")
        } else {
            format!("`${name}`'s fields can't change")
        };
        let refusal = match known.role {
            Role::Const => Some((
                "a `const` never changes (D30)",
                format!("`${name}` is declared `const` here"),
            )),
            Role::VarImmut => Some((
                "a `var.immut` never changes after it is set, nor do its fields (D30, D58)",
                format!("`${name}` is declared `var.immut` here"),
            )),
            Role::Parameter {
                changeability: Changeability::Fixed,
                holding: Holding::Lent | Holding::Given,
            } => Some((
                "a parameter is `var.immut` unless declared `var` or `var.mut`, or held `Lent.mut` (D31, D91)",
                format!("`${name}` is declared here"),
            )),
            Role::LoopValue => Some((
                "a `for` loop's own value is `var.immut` for each trip (D115)",
                format!("`${name}` is this loop's own value"),
            )),
            Role::CaseValue => Some((
                "a value pulled out by a `case` doesn't change (D72)",
                format!("`${name}` is pulled out here"),
            )),
            _ => None,
        };
        if let Some((why, fact)) = refusal {
            return Err(Problem::refused(place, what, why).because(known.place, fact));
        }
        if target.fields.is_empty() {
            if matches!(
                known.role,
                Role::Parameter {
                    holding: Holding::LentMut,
                    ..
                }
            ) {
                return Err(Problem::not_decided(
                    place,
                    "replacing a whole `Lent.mut` parameter",
                ));
            }
            if self.types.is_must_give(&known.kind) {
                return Err(Problem::not_decided(place, "replacing a `must_Give` value"));
            }
        }
        let target_kind = self.field_path(place, &known.kind, &target.fields)?;
        if let Some(known) = self.lookup_mut(name) {
            known.changed = true;
        }
        if target_kind != value_kind {
            return Err(Problem::refused(
                value.place,
                format!(
                    "this is {}, but it goes into {}",
                    value_kind.describe(),
                    target_kind.describe()
                ),
                "a value holds only values of its own type (D30, D58)",
            )
            .because(known.place, format!("`${name}` is declared here")));
        }
        Ok(())
    }

    /// The kind at the end of `$x.a.b`, refusing locked fields on the way (D58).
    fn field_path(
        &self,
        place: Place,
        root: &ValueKind,
        fields: &[String],
    ) -> Result<ValueKind, Problem> {
        let mut kind = root.clone();
        for field_name in fields {
            let ValueKind::Named(type_name) = &kind else {
                return Err(Problem::refused(
                    place,
                    format!("{} has no fields", kind.describe()),
                    "only a `struct` has fields (D58)",
                ));
            };
            let Some(definition) = self.types.structure(type_name) else {
                return Err(Problem::refused(
                    place,
                    format!("a `{type_name}` has no fields to change"),
                    "a `union.tagged`'s data is reached with `match` (D72)",
                ));
            };
            let Some(field) = definition
                .fields
                .iter()
                .find(|field| &field.name == field_name)
            else {
                return Err(Problem::refused(
                    place,
                    format!("a `{type_name}` has no field `{field_name}`"),
                    "a struct's fields are the ones it declares (D58)",
                )
                .because(definition.place, format!("`{type_name}` is declared here")));
            };
            if field.locked {
                return Err(Problem::refused(
                    place,
                    format!("`{field_name}` is locked"),
                    "a field written `.immut` never changes, whatever holds it (D58)",
                )
                .because(
                    field.place,
                    format!("`{field_name}` is declared `.immut` here"),
                ));
            }
            kind = field.type_spec.kind();
        }
        Ok(kind)
    }

    fn for_loop(
        &mut self,
        place: Place,
        loop_name: Option<&String>,
        variable: &str,
        range: &'program LoopRange,
        body: &'program [Statement],
    ) -> Result<(), Problem> {
        let end = match &range.end {
            RangeEnd::To(end) | RangeEnd::Before(end) => end,
        };
        self.integer(&range.from, "a range's `from:`")?;
        self.integer(end, "a range's end")?;
        if let Some(step) = &range.step_by {
            self.integer(step, "a range's `step_by:`")?;
        }
        let step_value = range
            .step_by
            .as_ref()
            .map_or(Some(BigInt::from(1)), literal_integer);
        if let (Some(from), Some(end), Some(step)) = (
            literal_integer(&range.from),
            literal_integer(end),
            step_value,
        ) && !range_can_end(&from, &end, &step)
        {
            return Err(Problem::refused(
                place,
                "this loop's step can never reach its end",
                "a loop's step must lead from `from:` to its end (D74)",
            ));
        }
        if let Some(first) = self.lookup(variable) {
            return Err(Problem::not_decided(
                place,
                format!(
                    "declaring `${variable}` again while the first is still visible (shadowing)"
                ),
            )
            .because(first.place, format!("the first `${variable}` is here")));
        }
        self.scopes.push(HashMap::new());
        self.declare(variable, Role::LoopValue, ValueKind::Integer, place);
        self.loop_body(loop_name, body)?;
        self.pop_scope()
    }

    /// Declares the values a `case` pulls out of its union (D72).
    fn bind_case(
        &mut self,
        union_name: &str,
        case: &MatchCase,
        declared: &Case,
    ) -> Result<(), Problem> {
        for binding in &case.bindings {
            let Some(field) = declared
                .fields
                .iter()
                .find(|field| field.name == binding.field)
            else {
                return Err(Problem::refused(
                    binding.place,
                    format!(
                        "`{union_name}.{}` has no field `{}`",
                        case.case_name, binding.field
                    ),
                    "a case pulls out the fields its case declares (D72)",
                )
                .because(declared.place, "the case is declared here"));
            };
            let field_kind = field.type_spec.kind();
            if !field_kind.is_copied() {
                return Err(Problem::not_decided(
                    binding.place,
                    "pulling out a field that is a struct, union or array",
                ));
            }
            if let Some(first) = self.lookup(&binding.name) {
                return Err(Problem::not_decided(
                    binding.place,
                    format!(
                        "declaring `${}` again while the first is still visible (shadowing)",
                        binding.name
                    ),
                )
                .because(
                    first.place,
                    format!("the first `${}` is here", binding.name),
                ));
            }
            self.declare(&binding.name, Role::CaseValue, field_kind, binding.place);
        }
        Ok(())
    }

    fn match_statement(
        &mut self,
        subject: &Expression,
        cases: &'program [MatchCase],
    ) -> Result<(), Problem> {
        let kind = self.place_kind(subject, Context::Plain)?;
        let ValueKind::Named(union_name) = &kind else {
            return Err(Problem::refused(
                subject.place,
                format!("this is {}, not a `union.tagged`", kind.describe()),
                "`match` chooses between the cases of a `union.tagged` (D72)",
            ));
        };
        let Some(definition) = self.types.union(union_name) else {
            return Err(Problem::refused(
                subject.place,
                format!("this is a `{union_name}`, not a `union.tagged`"),
                "`match` chooses between the cases of a `union.tagged` (D72)",
            ));
        };
        let before = self.scopes.clone();
        let mut outcomes = Vec::new();
        let mut covered: HashMap<&str, Place> = HashMap::new();
        for case in cases {
            if &case.type_name != union_name {
                return Err(Problem::refused(
                    case.place,
                    format!(
                        "this `case` is a `{}`, but the value is a `{union_name}`",
                        case.type_name
                    ),
                    "a case is written with its own type, the one being matched (D59, D72)",
                ));
            }
            let Some(declared) = self.types.case(union_name, &case.case_name) else {
                return Err(Problem::refused(
                    case.place,
                    format!("a `{union_name}` has no case `{}`", case.case_name),
                    "a `case` names one of the union's cases (D72)",
                )
                .because(definition.place, format!("`{union_name}` is declared here")));
            };
            if let Some(first) = covered.insert(case.case_name.as_str(), case.place) {
                return Err(Problem::refused(
                    case.place,
                    format!("a second `case {union_name}.{}`", case.case_name),
                    "each case is handled once; branches never fall through (D72)",
                )
                .because(first, "the first one is here"));
            }
            self.scopes.clone_from(&before);
            self.scopes.push(HashMap::new());
            self.bind_case(union_name, case, declared)?;
            let snapshot = self.scopes.clone();
            let outcome = self.branch(&snapshot, &case.body)?;
            self.pop_scope()?;
            if let Some(mut outcome) = outcome {
                outcome.pop();
                outcomes.push(outcome);
            }
        }
        let missing: Vec<&str> = definition
            .cases
            .iter()
            .map(|case| case.name.as_str())
            .filter(|name| !covered.contains_key(name))
            .collect();
        if !missing.is_empty() {
            let list = missing
                .iter()
                .map(|name| format!("`{union_name}.{name}`"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Problem::refused(
                subject.place,
                format!("this `match` doesn't cover {list}"),
                "every case must be covered (D72)",
            )
            .because(definition.place, format!("`{union_name}` is declared here")));
        }
        self.merge(before, &outcomes);
        Ok(())
    }
}

// Calls and the values handed to them.
impl Checker<'_> {
    /// Checks a call, giving back the kind of its result, if it has one.
    fn call(&mut self, call: &Call, context: Context) -> Result<Option<ValueKind>, Problem> {
        let in_pure = self.function.is_some_and(|function| function.pure);
        let (name, marked) = match &call.target {
            CallTarget::Print => {
                self.print(call, in_pure)?;
                return Ok(None);
            }
            CallTarget::Own { name, marked } => (name, *marked),
        };
        let Some(function) = self.functions.get(name.as_str()).copied() else {
            return Err(Problem::refused(
                call.place,
                format!("there is no function named `{name}`"),
                "a call names a function the program declares (D50)",
            ));
        };
        if is_radium_word(name) && !marked {
            return Err(Problem::refused(
                call.place,
                format!("`{name}` is also a Radium word"),
                "a writer's function whose name clashes with a Radium word is called with `my.` (D43)",
            )
            .fix(format!("`my.{name}(…)`")));
        }
        if marked && !is_radium_word(name) {
            return Err(Problem::not_decided(
                call.place,
                "whether `my.` may be written when no name clashes (D43)",
            ));
        }
        if in_pure && !function.pure {
            return Err(Problem::refused(
                call.place,
                format!("`{name}` isn't `fn.pure`"),
                "a `fn.pure` calls only other `fn.pure` functions (D51)",
            )
            .because(function.place, format!("`{name}` is declared here")));
        }
        if context != Context::Plain && !function.pure {
            return Err(Problem::refused(
                call.place,
                format!("`{name}` isn't `fn.pure`"),
                "a contract calls only `fn.pure` functions, so it never does anything (D51)",
            )
            .because(function.place, format!("`{name}` is declared here")));
        }
        let bindings = bind_arguments(function, call)?;
        let mut lent_to_change: Vec<(&str, Place)> = Vec::new();
        for (parameter, binding) in function.parameters.iter().zip(bindings) {
            let Binding::Argument(index) = binding else {
                continue;
            };
            let Some(argument) = call.arguments.get(index) else {
                continue;
            };
            self.argument(parameter, argument, context, &mut lent_to_change)?;
        }
        Ok(function.result.as_ref().map(TypeSpec::kind))
    }

    fn print(&mut self, call: &Call, in_pure: bool) -> Result<(), Problem> {
        if in_pure {
            return Err(Problem::refused(
                call.place,
                "printing inside a `fn.pure`",
                "a `fn.pure` has no side effects (D51)",
            )
            .because(
                self.function_place(),
                "this function is declared `fn.pure` here",
            ));
        }
        for argument in &call.arguments {
            if argument.label.is_some() {
                return Err(Problem::refused(
                    argument.place,
                    "a label on a `std::print` item",
                    "`std::print`'s items all have the same role, so they take no labels (D54)",
                ));
            }
            if let Some(handoff) = argument.handoff {
                return Err(Problem::not_decided(
                    argument.place,
                    format!("`{}` on a `std::print` item", handoff.word()),
                ));
            }
            if !matches!(
                argument.value.kind,
                ExpressionKind::Text(_) | ExpressionKind::Newline
            ) {
                let kind = self.place_kind(&argument.value, Context::Plain)?;
                if !kind.is_copied() {
                    return Err(Problem::not_decided(
                        argument.value.place,
                        "how a struct, union or array is printed",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Checks one argument against its parameter, including how it is handed over (D80, D91, D92).
    fn argument<'call>(
        &mut self,
        parameter: &Parameter,
        argument: &'call Argument,
        context: Context,
        lent_to_change: &mut Vec<(&'call str, Place)>,
    ) -> Result<(), Problem> {
        let wanted = parameter.type_spec.kind();
        let declared = (
            parameter.place,
            format!("`${}` is declared here", parameter.name),
        );
        if wanted.is_copied() {
            if let Some(handoff) = argument.handoff {
                return Err(Problem::refused(
                    argument.place,
                    format!("`{}` on a value that is always copied", handoff.word()),
                    "integers and `bool` are always copied, so they take no ownership word (D92)",
                ));
            }
            let kind = self.expression(&argument.value, context)?;
            return refuse_mismatch(argument, &kind, &wanted, declared);
        }
        let Some(handoff) = argument.handoff else {
            return Err(Problem::not_decided(
                argument.value.place,
                "passing a struct, union or array without `Give`, `Copy`, `Lend` or `Lend.mut`",
            ));
        };
        let ExpressionKind::Value(name) = &argument.value.kind else {
            return Err(Problem::not_decided(
                argument.value.place,
                "handing over something other than a whole value's `$name`",
            ));
        };
        let Some(known) = self.lookup(name).cloned() else {
            return Err(undeclared(argument.value.place, name));
        };
        refuse_given(argument.value.place, name, &known)?;
        refuse_mismatch(argument, &known.kind, &wanted, declared.clone())?;
        if handoff == Handoff::Copy && self.types.is_must_give(&known.kind) {
            return Err(Problem::refused(
                argument.place,
                format!("`${name}` can't be copied"),
                "a `must_Give` value is handed off exactly once, so there is never a second one (D84)",
            )
            .because(known.place, format!("`${name}` is declared here")));
        }
        refuse_pairing(parameter, handoff, argument, name, declared)?;
        if let Some((_, first)) = lent_to_change
            .iter()
            .find(|(other, _)| *other == name.as_str())
        {
            return Err(Problem::refused(
                argument.value.place,
                format!("`${name}` is lent to change, and used again in the same call"),
                "a value lent with `Lend.mut` is used by nothing else while it is lent (D5)",
            )
            .because(*first, "it is lent to change here"));
        }
        match handoff {
            Handoff::LendMut => {
                if !known.role.may_change() {
                    return Err(Problem::refused(
                        argument.place,
                        format!("`${name}` can't be lent to change"),
                        "`Lend.mut` lends a value that may change; this one can't (D30, D31, D80)",
                    )
                    .because(known.place, format!("`${name}` is declared here")));
                }
                lent_to_change.push((name.as_str(), argument.place));
                if let Some(known) = self.lookup_mut(name) {
                    known.changed = true;
                }
            }
            Handoff::Give => {
                if let Some(known) = self.lookup_mut(name) {
                    known.given = Some(Given::Yes(argument.place));
                }
            }
            Handoff::Copy | Handoff::Lend => {
                if let Some(other) = lent_to_change
                    .iter()
                    .find(|(other, _)| *other == name.as_str())
                {
                    return Err(Problem::refused(
                        argument.value.place,
                        format!("`${name}` is lent to change, and used again in the same call"),
                        "a value lent with `Lend.mut` is used by nothing else while it is lent (D5)",
                    )
                    .because(other.1, "it is lent to change here"));
                }
            }
        }
        Ok(())
    }
}

/// Refuses an argument of the wrong kind.
fn refuse_mismatch(
    argument: &Argument,
    kind: &ValueKind,
    wanted: &ValueKind,
    declared: (Place, String),
) -> Result<(), Problem> {
    if kind == wanted {
        return Ok(());
    }
    Err(Problem::refused(
        argument.value.place,
        format!(
            "this is {}, but the parameter takes {}",
            kind.describe(),
            wanted.describe()
        ),
        "an argument has its parameter's type (D50)",
    )
    .because(declared.0, declared.1))
}

/// Refuses a call word that doesn't match how the parameter holds the value (D91, D92).
fn refuse_pairing(
    parameter: &Parameter,
    handoff: Handoff,
    argument: &Argument,
    name: &str,
    declared: (Place, String),
) -> Result<(), Problem> {
    let matches = match parameter.holding {
        Holding::Lent => handoff == Handoff::Lend,
        Holding::LentMut => handoff == Handoff::LendMut,
        Holding::Given => matches!(handoff, Handoff::Give | Handoff::Copy),
    };
    if matches {
        return Ok(());
    }
    let (held, fix) = match parameter.holding {
        Holding::Lent => ("lent", Some(format!("`Lend ${name}`"))),
        Holding::LentMut => ("`Lent.mut`", Some(format!("`Lend.mut ${name}`"))),
        Holding::Given => ("`Given`", None),
    };
    let problem = Problem::refused(
        argument.place,
        format!(
            "`${}` is {held}, but this call says `{}`",
            parameter.name,
            handoff.word()
        ),
        "the call's word matches how the parameter holds it: `Lend` for a bare parameter, `Lend.mut` for `Lent.mut`, `Give` or `Copy` for `Given` (D91, D92)",
    );
    let problem = match fix {
        Some(fix) => problem.fix(fix),
        None => problem,
    };
    Err(problem.because(declared.0, declared.1))
}

/// Refuses using a value after it was handed off with `Give` (D80).
fn refuse_given(place: Place, name: &str, known: &Known) -> Result<(), Problem> {
    match known.given {
        None => Ok(()),
        Some(Given::Yes(given)) => Err(Problem::refused(
            place,
            format!("`${name}` can't be used here"),
            "a value handed over with `Give` can't be used afterwards (D80, D84)",
        )
        .because(given, format!("`${name}` was handed over here"))),
        Some(Given::Maybe(given)) => Err(Problem::refused(
            place,
            format!("`${name}` may already have been handed over"),
            "a value handed over with `Give` can't be used afterwards, on any path (D80, D84)",
        )
        .because(given, format!("`${name}` is handed over here, on one path"))),
    }
}

// Expressions.
impl Checker<'_> {
    fn condition(
        &mut self,
        condition: &Expression,
        context: Context,
        what: &str,
    ) -> Result<(), Problem> {
        if self.expression(condition, context)? == ValueKind::Bool {
            Ok(())
        } else {
            Err(Problem::refused(
                condition.place,
                format!("{what}'s condition isn't a `bool`"),
                "a condition is a `bool`; numbers never count as true or false (D60)",
            ))
        }
    }

    fn integer(&mut self, expression: &Expression, what: &str) -> Result<(), Problem> {
        self.integer_in(expression, Context::Plain, what)
    }

    fn integer_in(
        &mut self,
        expression: &Expression,
        context: Context,
        what: &str,
    ) -> Result<(), Problem> {
        if self.expression(expression, context)? == ValueKind::Integer {
            Ok(())
        } else {
            Err(Problem::refused(
                expression.place,
                format!("{what} isn't an integer"),
                "this place takes an integer (D73, D75)",
            ))
        }
    }

    /// The kind of a value that is only read in place: a field's base, or what a `match` looks at. Unlike
    /// `expression`, a whole struct or union may be named here.
    fn place_kind(
        &mut self,
        expression: &Expression,
        context: Context,
    ) -> Result<ValueKind, Problem> {
        if let ExpressionKind::Value(name) = &expression.kind {
            let Some(known) = self.lookup(name) else {
                return Err(undeclared(expression.place, name));
            };
            refuse_given(expression.place, name, known)?;
            return Ok(known.kind.clone());
        }
        self.expression(expression, context)
    }

    fn expression(
        &mut self,
        expression: &Expression,
        context: Context,
    ) -> Result<ValueKind, Problem> {
        let place = expression.place;
        match &expression.kind {
            ExpressionKind::Integer(_) => Ok(ValueKind::Integer),
            ExpressionKind::Bool(_) => Ok(ValueKind::Bool),
            ExpressionKind::Text(_) | ExpressionKind::Newline => Err(Problem::not_decided(
                place,
                "text outside `std::print`; text types are open beyond D120",
            )),
            ExpressionKind::Value(name) => {
                let kind = self.place_kind(expression, context)?;
                if !kind.is_copied() {
                    return Err(Problem::not_decided(
                        place,
                        format!(
                            "using a whole `${name}` here, without `Give`, `Copy` or `Lend` in a call"
                        ),
                    ));
                }
                Ok(kind)
            }
            ExpressionKind::Old(name) => self.old(place, name, context),
            ExpressionKind::Result => {
                let result = self.function.and_then(|function| function.result.as_ref());
                match (context, result) {
                    (Context::Ensures, Some(result)) => Ok(result.kind()),
                    _ => Err(Problem::refused(
                        place,
                        "`result` here",
                        "`result` is the value given back, so it only appears in an `ensures` of a function with a result (D53)",
                    )),
                }
            }
            ExpressionKind::Call(call) => self.call(call, context)?.ok_or_else(|| {
                Problem::refused(
                    place,
                    "this call gives nothing back to use as a value",
                    "a `-> void` function gives nothing back (D99)",
                )
            }),
            ExpressionKind::Build {
                type_name,
                case,
                fields,
            } => self.build(place, type_name, case.as_deref(), fields, context),
            ExpressionKind::Field { base, name } => self.field_read(place, base, name, context),
            ExpressionKind::Negate(inner) => self.operands(
                place,
                context,
                &[inner],
                &ValueKind::Integer,
                "`-`",
                ValueKind::Integer,
            ),
            ExpressionKind::Not(inner) => self.operands(
                place,
                context,
                &[inner],
                &ValueKind::Bool,
                "`not`",
                ValueKind::Bool,
            ),
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } => self.binary(place, context, *operator, left, right),
        }
    }

    fn old(&mut self, place: Place, name: &str, context: Context) -> Result<ValueKind, Problem> {
        if context != Context::Ensures {
            return Err(Problem::refused(
                place,
                "`old[…]` outside an `ensures`",
                "`old[$x]` is a parameter's starting value, which only an `ensures` talks about (D53)",
            ));
        }
        match self.lookup(name) {
            Some(known) if matches!(known.role, Role::Parameter { .. }) => {
                if known.kind.is_copied() {
                    Ok(known.kind.clone())
                } else {
                    Err(Problem::not_decided(
                        place,
                        "`old[…]` of a struct, union or array",
                    ))
                }
            }
            _ => Err(Problem::refused(
                place,
                format!("`${name}` isn't a parameter"),
                "`old[…]` takes a parameter (D53)",
            )),
        }
    }

    /// `reading[time = 3600]`, `status.ok` or `status.failed[code = 3]` (D58, D59).
    fn build(
        &mut self,
        place: Place,
        type_name: &str,
        case: Option<&str>,
        values: &[FieldValue],
        context: Context,
    ) -> Result<ValueKind, Problem> {
        let (declared_fields, declared_at) = match (
            case,
            self.types.structure(type_name),
            self.types.union(type_name),
        ) {
            (None, Some(definition), _) => (&definition.fields, definition.place),
            (Some(case_name), _, Some(definition)) => {
                let Some(declared) = self.types.case(type_name, case_name) else {
                    return Err(Problem::refused(
                        place,
                        format!("a `{type_name}` has no case `{case_name}`"),
                        "a case is one the union declares (D59)",
                    )
                    .because(definition.place, format!("`{type_name}` is declared here")));
                };
                (&declared.fields, declared.place)
            }
            (None, None, Some(definition)) => {
                return Err(Problem::refused(
                    place,
                    format!("a `{type_name}` is built as one of its cases"),
                    "a `union.tagged` value is one of its cases, written with its type: `status.ok` (D59)",
                )
                .because(definition.place, format!("`{type_name}` is declared here")));
            }
            (Some(_), Some(definition), _) => {
                return Err(Problem::refused(
                    place,
                    format!("`{type_name}` is a struct, which has no cases"),
                    "a struct is built with its fields: `{type_name}[field = …]` (D58)",
                )
                .because(definition.place, format!("`{type_name}` is declared here")));
            }
            (_, None, None) => {
                return Err(Problem::refused(
                    place,
                    format!("there is no type named `{type_name}`"),
                    "a value is built from a declared `struct` or `union.tagged` (D58, D59)",
                ));
            }
        };
        for value in values {
            let Some(field) = declared_fields
                .iter()
                .find(|field| field.name == value.name)
            else {
                return Err(Problem::refused(
                    value.place,
                    format!("`{type_name}` has no field `{}`", value.name),
                    "a value is built from the fields its type declares (D58, D59)",
                )
                .because(declared_at, format!("`{type_name}` is declared here")));
            };
            if values
                .iter()
                .filter(|other| other.name == value.name)
                .count()
                > 1
            {
                return Err(Problem::refused(
                    value.place,
                    format!("`{}` is set twice", value.name),
                    "each field is set once (D58)",
                ));
            }
            let kind = self.expression(&value.value, context)?;
            let wanted = field.type_spec.kind();
            if kind != wanted {
                return Err(Problem::refused(
                    value.value.place,
                    format!(
                        "this is {}, but `{}` holds {}",
                        kind.describe(),
                        value.name,
                        wanted.describe()
                    ),
                    "a field holds only values of its own type (D58)",
                )
                .because(field.place, format!("`{}` is declared here", value.name)));
            }
        }
        if let Some(missing) = declared_fields
            .iter()
            .find(|field| !values.iter().any(|value| value.name == field.name))
        {
            return Err(Problem::not_decided(
                place,
                format!(
                    "building a `{type_name}` without setting `{}`",
                    missing.name
                ),
            ));
        }
        Ok(ValueKind::Named(type_name.to_owned()))
    }

    /// `$r.celsius` (D26, D58).
    fn field_read(
        &mut self,
        place: Place,
        base: &Expression,
        name: &str,
        context: Context,
    ) -> Result<ValueKind, Problem> {
        let base_kind = self.place_kind(base, context)?;
        let ValueKind::Named(type_name) = &base_kind else {
            return Err(Problem::refused(
                place,
                format!("{} has no fields", base_kind.describe()),
                "only a `struct` has fields (D58)",
            ));
        };
        let Some(definition) = self.types.structure(type_name) else {
            return Err(Problem::refused(
                place,
                format!("a `{type_name}`'s data isn't read with `.`"),
                "a `union.tagged`'s data is reached with `match` (D72)",
            ));
        };
        let Some(field) = definition.fields.iter().find(|field| field.name == name) else {
            return Err(Problem::refused(
                place,
                format!("a `{type_name}` has no field `{name}`"),
                "a struct's fields are the ones it declares (D58)",
            )
            .because(definition.place, format!("`{type_name}` is declared here")));
        };
        Ok(field.type_spec.kind())
    }

    fn binary(
        &mut self,
        place: Place,
        context: Context,
        operator: Operator,
        left: &Expression,
        right: &Expression,
    ) -> Result<ValueKind, Problem> {
        let sides = [left, right];
        match operator {
            Operator::Add
            | Operator::Subtract
            | Operator::Multiply
            | Operator::Divide(_)
            | Operator::Remainder(_) => self.operands(
                place,
                context,
                &sides,
                &ValueKind::Integer,
                "arithmetic",
                ValueKind::Integer,
            ),
            Operator::Less | Operator::Greater | Operator::AtMost | Operator::AtLeast => self
                .operands(
                    place,
                    context,
                    &sides,
                    &ValueKind::Integer,
                    "an ordering comparison",
                    ValueKind::Bool,
                ),
            Operator::And | Operator::Or => self.operands(
                place,
                context,
                &sides,
                &ValueKind::Bool,
                "`and`/`or`",
                ValueKind::Bool,
            ),
            Operator::Equal | Operator::NotEqual => {
                let left_kind = self.place_kind(left, context)?;
                let right_kind = self.place_kind(right, context)?;
                if !left_kind.is_copied() || !right_kind.is_copied() {
                    return Err(Problem::not_decided(
                        place,
                        "comparing structs, unions or arrays with `equal`",
                    ));
                }
                if left_kind == right_kind {
                    Ok(ValueKind::Bool)
                } else {
                    Err(Problem::refused(
                        place,
                        "these two values are different kinds",
                        "`equal` and `not_equal` compare two values of the same kind (D34)",
                    ))
                }
            }
        }
    }

    fn operands(
        &mut self,
        place: Place,
        context: Context,
        operands: &[&Expression],
        wanted: &ValueKind,
        what: &str,
        gives: ValueKind,
    ) -> Result<ValueKind, Problem> {
        for operand in operands {
            let kind = self.expression(operand, context)?;
            if &kind != wanted {
                return Err(Problem::refused(
                    operand.place,
                    format!(
                        "this is {}, but {what} needs {}",
                        kind.describe(),
                        wanted.describe()
                    ),
                    "each operator works on one kind of value (D34, D60)",
                )
                .because(place, "the operator is here"));
            }
        }
        Ok(gives)
    }

    fn is_constant(&self, expression: &Expression) -> bool {
        match &expression.kind {
            ExpressionKind::Integer(_) | ExpressionKind::Bool(_) => true,
            ExpressionKind::Value(name) => self
                .lookup(name)
                .is_some_and(|known| known.role == Role::Const),
            ExpressionKind::Negate(inner) | ExpressionKind::Not(inner) => self.is_constant(inner),
            ExpressionKind::Binary { left, right, .. } => {
                self.is_constant(left) && self.is_constant(right)
            }
            ExpressionKind::Text(_)
            | ExpressionKind::Newline
            | ExpressionKind::Old(_)
            | ExpressionKind::Result
            | ExpressionKind::Call(_)
            | ExpressionKind::Build { .. }
            | ExpressionKind::Field { .. } => false,
        }
    }

    /// The place of the function being checked.
    fn function_place(&self) -> Place {
        self.function.map_or(
            Place {
                line: 1,
                byte_column: 1,
                column: 1,
                width: 1,
            },
            |function| function.place,
        )
    }

    fn declare(&mut self, name: &str, role: Role, kind: ValueKind, place: Place) {
        let known = Known {
            role,
            kind,
            place,
            changed: false,
            given: None,
        };
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_owned(), known);
        }
    }

    fn lookup(&self, name: &str) -> Option<&Known> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut Known> {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(name))
    }
}

fn labels_and_defaults(function: &Function) -> Result<(), Problem> {
    if let Some(labels) = &function.labels {
        for (index, entry) in labels.entries.iter().enumerate() {
            if !function
                .parameters
                .iter()
                .any(|parameter| parameter.name == entry.parameter)
            {
                return Err(Problem::refused(
                    labels.place,
                    format!("`${}` isn't a parameter", entry.parameter),
                    "`labels` gives labels to the function's own parameters (D54)",
                ));
            }
            let earlier = labels.entries.iter().take(index);
            if earlier
                .clone()
                .any(|other| other.parameter == entry.parameter)
            {
                return Err(Problem::refused(
                    labels.place,
                    format!("`${}` is labelled twice", entry.parameter),
                    "each parameter has at most one label (D54)",
                ));
            }
            if earlier.clone().any(|other| other.label == entry.label) {
                return Err(Problem::refused(
                    labels.place,
                    format!("two parameters share the label `{}`", entry.label),
                    "each label names one parameter (D54)",
                ));
            }
        }
    }
    for (index, default) in function.defaults.iter().enumerate() {
        let labelled = function.labels.as_ref().is_some_and(|labels| {
            labels
                .entries
                .iter()
                .any(|entry| entry.parameter == default.parameter)
        });
        if !labelled {
            return Err(Problem::refused(
                default.place,
                format!("`${}` has a default but no label", default.parameter),
                "only labelled parameters may have defaults (D54)",
            ));
        }
        if function
            .defaults
            .iter()
            .take(index)
            .any(|other| other.parameter == default.parameter)
        {
            return Err(Problem::refused(
                default.place,
                format!("`${}` has two defaults", default.parameter),
                "each parameter has at most one default (D54)",
            ));
        }
        let Some(kind) = literal_kind(&default.value) else {
            return Err(Problem::not_decided(
                default.place,
                "defaults other than a plain number, `true` or `false`",
            ));
        };
        let parameter = function
            .parameters
            .iter()
            .find(|parameter| parameter.name == default.parameter);
        if parameter.is_some_and(|parameter| parameter.type_spec.kind() != kind) {
            return Err(Problem::refused(
                default.value.place,
                format!(
                    "this default is {}, which `${}`'s type doesn't hold",
                    kind.describe(),
                    default.parameter
                ),
                "a default has its parameter's type (D54)",
            ));
        }
    }
    Ok(())
}

/// Whether a block always ends in `return`, whichever way it goes: its last statement returns, or is an `if` with an
/// `else`, or a `match`, whose every branch always returns, or is a `forever` that nothing breaks out of.
fn always_returns(statements: &[Statement]) -> bool {
    match statements.last().map(|statement| &statement.kind) {
        Some(StatementKind::Return { .. }) => true,
        Some(StatementKind::If {
            branches,
            else_body: Some(else_body),
        }) => {
            branches.iter().all(|branch| always_returns(&branch.body)) && always_returns(else_body)
        }
        Some(StatementKind::Match { cases, .. }) => {
            cases.iter().all(|case| always_returns(&case.body))
        }
        Some(StatementKind::Forever { loop_name, body }) => {
            !breaks_out(body, loop_name.as_ref(), 0)
        }
        _ => false,
    }
}

/// Whether a block's last statement is a `break`.
fn ends_in_break(statements: &[Statement]) -> bool {
    matches!(
        statements.last().map(|statement| &statement.kind),
        Some(StatementKind::Break { .. })
    )
}

/// Whether some `break` in `body` leaves the loop that owns it: a bare `break` not inside a nested loop, or a
/// `break` naming that loop.
fn breaks_out(body: &[Statement], loop_name: Option<&String>, depth: usize) -> bool {
    body.iter().any(|statement| match &statement.kind {
        StatementKind::Break { loop_name: target } => match target {
            None => depth == 0,
            Some(target) => loop_name == Some(target),
        },
        StatementKind::If {
            branches,
            else_body,
        } => {
            branches
                .iter()
                .any(|branch| breaks_out(&branch.body, loop_name, depth))
                || else_body
                    .as_ref()
                    .is_some_and(|body| breaks_out(body, loop_name, depth))
        }
        StatementKind::Match { cases, .. } => cases
            .iter()
            .any(|case| breaks_out(&case.body, loop_name, depth)),
        StatementKind::For { body, .. }
        | StatementKind::While { body, .. }
        | StatementKind::Forever { body, .. } => {
            breaks_out(body, loop_name, depth.saturating_add(1))
        }
        StatementKind::Declare { .. }
        | StatementKind::Assign { .. }
        | StatementKind::Call(_)
        | StatementKind::Return { .. } => false,
    })
}

fn undeclared(place: Place, name: &str) -> Problem {
    Problem::refused(
        place,
        format!("`${name}` isn't declared"),
        "a value is declared before it is used (D30, D31)",
    )
}

fn literal_integer(expression: &Expression) -> Option<BigInt> {
    match &expression.kind {
        ExpressionKind::Integer(number) => Some(number.clone()),
        ExpressionKind::Negate(inner) => literal_integer(inner).map(|number| -number),
        _ => None,
    }
}

fn literal_kind(expression: &Expression) -> Option<ValueKind> {
    match &expression.kind {
        ExpressionKind::Bool(_) => Some(ValueKind::Bool),
        _ => literal_integer(expression).map(|_| ValueKind::Integer),
    }
}
// end section oracle.checker
