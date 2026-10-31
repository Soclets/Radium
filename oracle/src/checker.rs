// section oracle.checker:
//! Refuses what the compiler would refuse, before anything runs: names and kinds of values, changes to values that
//! may not change (D30, D31), conditions that aren't `bool` (D60), `break` outside a loop (D77), `return` and
//! `result` (D53, D95), and calls: labels, defaults, `my.` and `fn.pure` (D43, D51, D54).

use std::collections::HashMap;

use crate::ast::{
    Call, CallTarget, Changeability, Expression, ExpressionKind, Function, Holder, LoopRange,
    Operator, Program, RangeEnd, Statement, StatementKind, TypeSpec, ValueKind,
};
use num_bigint::BigInt;

use crate::binding::{Binding, bind_arguments};
use crate::outcome::{Place, Problem};
use crate::rules::{is_radium_word, range_can_end};

/// Checks a whole program.
pub(crate) fn check(program: &Program) -> Result<(), Problem> {
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
    Parameter(Changeability),
    LoopValue,
}

#[derive(Debug, Clone, Copy)]
struct Known {
    role: Role,
    kind: ValueKind,
    /// Where it was declared, for `because:` links (D122).
    place: Place,
    changed: bool,
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

struct Checker<'program> {
    functions: HashMap<&'program str, &'program Function>,
    scopes: Vec<HashMap<String, Known>>,
    loops: Vec<Option<String>>,
    function: Option<&'program Function>,
}

impl<'program> Checker<'program> {
    fn function_definition(&mut self, function: &'program Function) -> Result<(), Problem> {
        self.function = Some(function);
        self.scopes.push(HashMap::new());
        for parameter in &function.parameters {
            check_type(&parameter.type_spec, parameter.place)?;
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
                Role::Parameter(parameter.changeability),
                parameter.type_spec.kind(),
                parameter.place,
            );
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

    fn block(&mut self, statements: &'program [Statement]) -> Result<(), Problem> {
        self.scopes.push(HashMap::new());
        for statement in statements {
            self.statement(statement)?;
        }
        self.pop_scope()
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
            StatementKind::Assign { name, value } => self.assignment(place, name, value),
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
                for branch in branches {
                    self.condition(&branch.condition, Context::Plain, "an `if`")?;
                    self.block(&branch.body)?;
                }
                if let Some(body) = else_body {
                    self.block(body)?;
                }
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
                    self.integer_in(measure, Context::Contract, "a `decreases` (D75)")?;
                }
                self.loop_body(None, body)
            }
            StatementKind::Forever { loop_name, body } => self.loop_body(loop_name.as_ref(), body),
            StatementKind::Break { loop_name } => self.break_statement(place, loop_name.as_ref()),
            StatementKind::Return { value } => self.return_statement(place, value.as_ref()),
        }
    }

    fn return_statement(&self, place: Place, value: Option<&Expression>) -> Result<(), Problem> {
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
            (None, None) => Ok(()),
            (None, Some(_)) => Err(Problem::refused(
                place,
                format!("`{}` gives nothing back", function.name),
                "a `-> void` function returns with `return void;` (D99, D100)",
            )
            .fix("`return void;`")
            .because(header.0, header.1)),
            (Some(result), None) => Err(Problem::refused(
                place,
                format!(
                    "`{}` gives back {}",
                    function.name,
                    result.kind().describe()
                ),
                "`return void;` is only for `-> void` functions (D100)",
            )
            .because(header.0, header.1)),
            (Some(result), Some(value)) => {
                let kind = self.expression(value, Context::Plain)?;
                if kind == result.kind() {
                    Ok(())
                } else {
                    Err(Problem::refused(
                        value.place,
                        format!(
                            "this is {}, but `{}` gives back {}",
                            kind.describe(),
                            function.name,
                            result.kind().describe()
                        ),
                        "a `return` gives back a value of the function's result type (D52, D95)",
                    )
                    .because(header.0, header.1))
                }
            }
        }
    }

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
            check_type(type_spec, place)?;
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

    fn assignment(&mut self, place: Place, name: &str, value: &Expression) -> Result<(), Problem> {
        let value_kind = self.expression(value, Context::Plain)?;
        let Some(known) = self.lookup_mut(name) else {
            return Err(undeclared(place, name));
        };
        let refusal = match known.role {
            Role::Const => Some((
                format!("`${name}` can't change"),
                "a `const` never changes (D30)",
                format!("`${name}` is declared `const` here"),
            )),
            Role::VarImmut => Some((
                format!("`${name}` can't change"),
                "a `var.immut` never changes after it is set (D30)",
                format!("`${name}` is declared `var.immut` here"),
            )),
            Role::Parameter(Changeability::Fixed) => Some((
                format!("`${name}` can't change"),
                "a parameter is `var.immut` unless declared `var` or `var.mut` (D31)",
                format!("`${name}` is declared here"),
            )),
            Role::LoopValue => Some((
                format!("`${name}` can't change inside its loop"),
                "a `for` loop's own value is `var.immut` for each trip (D115)",
                format!("`${name}` is this loop's own value"),
            )),
            Role::Var | Role::VarMut | Role::Parameter(_) => None,
        };
        if let Some((what, why, fact)) = refusal {
            return Err(Problem::refused(place, what, why).because(known.place, fact));
        }
        known.changed = true;
        if known.kind != value_kind {
            return Err(Problem::refused(
                value.place,
                format!(
                    "this is {}, but `${name}` holds {}",
                    value_kind.describe(),
                    known.kind.describe()
                ),
                "a value holds only values of its own type (D30)",
            )
            .because(known.place, format!("`${name}` is declared here")));
        }
        Ok(())
    }

    /// Checks a call, giving back the kind of its result, if it has one.
    fn call(&self, call: &Call, context: Context) -> Result<Option<ValueKind>, Problem> {
        let in_pure = self.function.is_some_and(|function| function.pure);
        match &call.target {
            CallTarget::Print => {
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
                    if !matches!(
                        argument.value.kind,
                        ExpressionKind::Text(_) | ExpressionKind::Newline
                    ) {
                        self.expression(&argument.value, Context::Plain)?;
                    }
                }
                Ok(None)
            }
            CallTarget::Own { name, marked } => {
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
                if *marked && !is_radium_word(name) {
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
                for (parameter, binding) in function.parameters.iter().zip(bindings) {
                    let Binding::Argument(index) = binding else {
                        continue;
                    };
                    let Some(argument) = call.arguments.get(index) else {
                        continue;
                    };
                    let kind = self.expression(&argument.value, context)?;
                    if kind != parameter.type_spec.kind() {
                        return Err(Problem::refused(
                            argument.value.place,
                            format!(
                                "this is {}, but `${}` takes {}",
                                kind.describe(),
                                parameter.name,
                                parameter.type_spec.kind().describe()
                            ),
                            "an argument has its parameter's type (D50)",
                        )
                        .because(
                            parameter.place,
                            format!("`${}` is declared here", parameter.name),
                        ));
                    }
                }
                Ok(function.result.as_ref().map(TypeSpec::kind))
            }
        }
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
        self.integer(&range.from, "a range's `from:` (D73)")?;
        self.integer(end, "a range's end (D73)")?;
        if let Some(step) = &range.step_by {
            self.integer(step, "a range's `step_by:` (D73)")?;
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

    fn loop_body(
        &mut self,
        loop_name: Option<&String>,
        body: &'program [Statement],
    ) -> Result<(), Problem> {
        if let Some(name) = loop_name
            && self.loops.iter().any(|other| other.as_ref() == Some(name))
        {
            return Err(Problem::not_decided(
                body.first()
                    .map_or(self.function_place(), |statement| statement.place),
                format!("a loop named `{name}` inside another loop named `{name}`"),
            ));
        }
        self.loops.push(loop_name.cloned());
        let result = self.block(body);
        self.loops.pop();
        result
    }

    fn break_statement(&self, place: Place, loop_name: Option<&String>) -> Result<(), Problem> {
        if self.loops.is_empty() {
            return Err(Problem::refused(
                place,
                "`break` with no loop around it",
                "`break` stops the loop it is in (D77)",
            ));
        }
        if let Some(name) = loop_name
            && !self.loops.iter().any(|other| other.as_ref() == Some(name))
        {
            return Err(Problem::refused(
                place,
                format!("there is no loop named `{name}` around this `break`"),
                "`break name;` stops the named loop it is in (D77)",
            ));
        }
        Ok(())
    }

    fn condition(
        &self,
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

    fn integer(&self, expression: &Expression, what: &str) -> Result<(), Problem> {
        self.integer_in(expression, Context::Plain, what)
    }

    fn integer_in(
        &self,
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

    fn expression(&self, expression: &Expression, context: Context) -> Result<ValueKind, Problem> {
        let place = expression.place;
        match &expression.kind {
            ExpressionKind::Integer(_) => Ok(ValueKind::Integer),
            ExpressionKind::Bool(_) => Ok(ValueKind::Bool),
            ExpressionKind::Text(_) | ExpressionKind::Newline => Err(Problem::not_decided(
                place,
                "text outside `std::print`; text types are open beyond D120",
            )),
            ExpressionKind::Value(name) => self
                .lookup(name)
                .map(|known| known.kind)
                .ok_or_else(|| undeclared(place, name)),
            ExpressionKind::Old(name) => {
                if context != Context::Ensures {
                    return Err(Problem::refused(
                        place,
                        "`old[…]` outside an `ensures`",
                        "`old[$x]` is a parameter's starting value, which only an `ensures` talks about (D53)",
                    ));
                }
                match self.lookup(name) {
                    Some(known) if matches!(known.role, Role::Parameter(_)) => Ok(known.kind),
                    _ => Err(Problem::refused(
                        place,
                        format!("`${name}` isn't a parameter"),
                        "`old[…]` takes a parameter (D53)",
                    )),
                }
            }
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
            ExpressionKind::Negate(inner) => self.operands(
                place,
                context,
                &[inner],
                ValueKind::Integer,
                "`-`",
                ValueKind::Integer,
            ),
            ExpressionKind::Not(inner) => self.operands(
                place,
                context,
                &[inner],
                ValueKind::Bool,
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

    fn binary(
        &self,
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
                ValueKind::Integer,
                "arithmetic",
                ValueKind::Integer,
            ),
            Operator::Less | Operator::Greater | Operator::AtMost | Operator::AtLeast => self
                .operands(
                    place,
                    context,
                    &sides,
                    ValueKind::Integer,
                    "an ordering comparison",
                    ValueKind::Bool,
                ),
            Operator::And | Operator::Or => self.operands(
                place,
                context,
                &sides,
                ValueKind::Bool,
                "`and`/`or`",
                ValueKind::Bool,
            ),
            Operator::Equal | Operator::NotEqual => {
                if self.expression(left, context)? == self.expression(right, context)? {
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
        &self,
        place: Place,
        context: Context,
        operands: &[&Expression],
        wanted: ValueKind,
        what: &str,
        gives: ValueKind,
    ) -> Result<ValueKind, Problem> {
        for operand in operands {
            let kind = self.expression(operand, context)?;
            if kind != wanted {
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
            | ExpressionKind::Call(_) => false,
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

    /// Leaves a scope, refusing any `var.mut` that nothing changed (D30).
    fn pop_scope(&mut self) -> Result<(), Problem> {
        let Some(scope) = self.scopes.pop() else {
            return Ok(());
        };
        let mut unchanged: Vec<(&String, &Known)> = scope
            .iter()
            .filter(|(_, known)| {
                !known.changed
                    && matches!(
                        known.role,
                        Role::VarMut | Role::Parameter(Changeability::MustChange)
                    )
            })
            .collect();
        unchanged.sort_by_key(|(_, known)| (known.place.line, known.place.column));
        if let Some((name, known)) = unchanged.first() {
            return Err(Problem::refused(
                known.place,
                format!("nothing changes `${name}`"),
                "a `var.mut` must change somewhere (D30)",
            ));
        }
        Ok(())
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
/// `else` whose every branch always returns, or is a `forever` that nothing breaks out of.
fn always_returns(statements: &[Statement]) -> bool {
    match statements.last().map(|statement| &statement.kind) {
        Some(StatementKind::Return { .. }) => true,
        Some(StatementKind::If {
            branches,
            else_body: Some(else_body),
        }) => {
            branches.iter().all(|branch| always_returns(&branch.body)) && always_returns(else_body)
        }
        Some(StatementKind::Forever { loop_name, body }) => {
            !breaks_out(body, loop_name.as_ref(), 0)
        }
        _ => false,
    }
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

fn check_type(type_spec: &TypeSpec, place: Place) -> Result<(), Problem> {
    if let TypeSpec::Integer(bounds) = type_spec
        && bounds.min > bounds.max
    {
        return Err(Problem::refused(
            place,
            format!("`min: {}` is above `max: {}`", bounds.min, bounds.max),
            "an integer's bounds go from `min` up to `max` (D14)",
        ));
    }
    Ok(())
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
