// section oracle.checker:
//! Refuses what the compiler would refuse, before anything runs: names and kinds of values, changes to values that
//! may not change (D30, D31), conditions that aren't `bool` (D60), `break` outside a loop (D77), `return` and
//! `result` (D53, D95), and calls: labels, defaults, `my.` and `fn.pure` (D43, D51, D54).

use std::collections::HashMap;

use crate::ast::{
    Call, CallTarget, Changeability, Expression, ExpressionKind, Function, Holder, LoopRange,
    Operator, Program, RangeEnd, Statement, StatementKind, TypeSpec, ValueKind,
};
use crate::binding::{Binding, bind_arguments};
use crate::outcome::Problem;
use crate::rules::{is_radium_word, range_can_end};

/// Checks a whole program.
pub(crate) fn check(program: &Program) -> Result<(), Problem> {
    let mut functions = HashMap::new();
    for function in &program.functions {
        if functions.insert(function.name.as_str(), function).is_some() {
            return Err(Problem::not_decided(
                function.line,
                format!(
                    "two functions named `{}`: whether a name may be used twice isn't decided",
                    function.name
                ),
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
            main.line,
            "parameters for `fn.main` (such as command-line arguments) aren't decided",
        ));
    }
    if main.result.is_some() {
        return Err(Problem::not_decided(
            main.line,
            "what `fn.main` may give back other than `void` (such as an exit status) isn't decided",
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
    line: usize,
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
            check_type(parameter.type_spec, parameter.line)?;
            if self.lookup(&parameter.name).is_some() {
                return Err(Problem::refused(
                    parameter.line,
                    format!("two parameters are named `${}`", parameter.name),
                ));
            }
            self.declare(
                &parameter.name,
                Role::Parameter(parameter.changeability),
                parameter.type_spec.kind(),
                parameter.line,
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
                function.line,
                format!(
                    "`{}` can reach its end without `return`; every path must end in `return`, which for `-> void` is `return void;` (D96, D102)",
                    function.name
                ),
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
        let line = statement.line;
        match &statement.kind {
            StatementKind::Declare {
                holder,
                name,
                type_spec,
                value,
            } => self.declaration(line, *holder, name, *type_spec, value),
            StatementKind::Assign { name, value } => self.assignment(line, name, value),
            StatementKind::Call(call) => {
                if self.call(call, Context::Plain)?.is_some() {
                    return Err(Problem::refused(
                        call.line,
                        "this call's result is thrown away; a result can't be ignored (D97)",
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
            } => self.for_loop(line, loop_name.as_ref(), variable, range, body),
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
            StatementKind::Break { loop_name } => self.break_statement(line, loop_name.as_ref()),
            StatementKind::Return { value } => self.return_statement(line, value.as_ref()),
        }
    }

    fn return_statement(&self, line: usize, value: Option<&Expression>) -> Result<(), Problem> {
        let Some(function) = self.function else {
            return Err(Problem::refused(line, "`return` is only inside a function"));
        };
        match (function.result, value) {
            (None, None) => Ok(()),
            (None, Some(_)) => Err(Problem::refused(
                line,
                format!(
                    "`{}` has `-> void`, so it gives nothing back; it returns with `return void;` (D99, D100)",
                    function.name
                ),
            )),
            (Some(result), None) => Err(Problem::refused(
                line,
                format!(
                    "`{}` gives back {}; `return void;` is only for `-> void` functions (D100)",
                    function.name,
                    result.kind().describe()
                ),
            )),
            (Some(result), Some(value)) => {
                let kind = self.expression(value, Context::Plain)?;
                if kind == result.kind() {
                    Ok(())
                } else {
                    Err(Problem::refused(
                        line,
                        format!(
                            "`{}` gives back {}, but this is {}",
                            function.name,
                            result.kind().describe(),
                            kind.describe()
                        ),
                    ))
                }
            }
        }
    }

    fn declaration(
        &mut self,
        line: usize,
        holder: Holder,
        name: &str,
        type_spec: Option<TypeSpec>,
        value: &Expression,
    ) -> Result<(), Problem> {
        let value_kind = self.expression(value, Context::Plain)?;
        if let Some(type_spec) = type_spec {
            check_type(type_spec, line)?;
            if type_spec.kind() != value_kind {
                return Err(Problem::refused(
                    line,
                    format!(
                        "`${name}` holds {}, but its value is {}",
                        type_spec.kind().describe(),
                        value_kind.describe()
                    ),
                ));
            }
        }
        if holder == Holder::Const && !self.is_constant(value) {
            return Err(Problem::refused(
                line,
                "a `const` is known when compiling, so its value may use only numbers, `true`, `false` and other `const`s (D30)",
            ));
        }
        if self.lookup(name).is_some() {
            return Err(Problem::not_decided(
                line,
                format!(
                    "declaring `${name}` again while the first one is still visible (shadowing) isn't decided"
                ),
            ));
        }
        let role = match holder {
            Holder::Var => Role::Var,
            Holder::VarImmut => Role::VarImmut,
            Holder::VarMut => Role::VarMut,
            Holder::Const => Role::Const,
        };
        self.declare(name, role, value_kind, line);
        Ok(())
    }

    fn assignment(&mut self, line: usize, name: &str, value: &Expression) -> Result<(), Problem> {
        let value_kind = self.expression(value, Context::Plain)?;
        let Some(known) = self.lookup_mut(name) else {
            return Err(Problem::refused(
                line,
                format!("`${name}` isn't declared (D31)"),
            ));
        };
        match known.role {
            Role::Const => {
                return Err(Problem::refused(
                    line,
                    format!("`${name}` is a `const`, which never changes (D30)"),
                ));
            }
            Role::VarImmut => {
                return Err(Problem::refused(
                    line,
                    format!("`${name}` is `var.immut`, so it never changes after it is set (D30)"),
                ));
            }
            Role::Parameter(Changeability::Fixed) => {
                return Err(Problem::refused(
                    line,
                    format!(
                        "`${name}` is a parameter, which is `var.immut` unless declared `var` or `var.mut` (D31)"
                    ),
                ));
            }
            Role::LoopValue => {
                return Err(Problem::not_decided(
                    line,
                    format!("whether a loop's own value `${name}` may be changed isn't decided"),
                ));
            }
            Role::Var | Role::VarMut | Role::Parameter(_) => {}
        }
        known.changed = true;
        if known.kind != value_kind {
            return Err(Problem::refused(
                line,
                format!(
                    "`${name}` holds {}, but the new value is {}",
                    known.kind.describe(),
                    value_kind.describe()
                ),
            ));
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
                        call.line,
                        "a `fn.pure` has no side effects, so it can't print (D51)",
                    ));
                }
                for argument in &call.arguments {
                    if argument.label.is_some() {
                        return Err(Problem::refused(
                            argument.line,
                            "`std::print`'s items take no labels (D54)",
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
                        call.line,
                        format!("there is no function named `{name}`"),
                    ));
                };
                if is_radium_word(name) && !marked {
                    return Err(Problem::refused(
                        call.line,
                        format!(
                            "`{name}` is also a Radium word, so your function is called `my.{name}` (D43)"
                        ),
                    ));
                }
                if *marked && !is_radium_word(name) {
                    return Err(Problem::not_decided(
                        call.line,
                        "whether `my.` may be written when no name clashes isn't decided (D43)",
                    ));
                }
                if in_pure && !function.pure {
                    return Err(Problem::refused(
                        call.line,
                        format!(
                            "a `fn.pure` can call only other `fn.pure` functions, and `{name}` isn't one (D51)"
                        ),
                    ));
                }
                if context != Context::Plain && !function.pure {
                    return Err(Problem::refused(
                        call.line,
                        format!(
                            "a contract may call only `fn.pure` functions, and `{name}` isn't one (D51)"
                        ),
                    ));
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
                            argument.line,
                            format!(
                                "`${}` takes {}, but this argument is {}",
                                parameter.name,
                                parameter.type_spec.kind().describe(),
                                kind.describe()
                            ),
                        ));
                    }
                }
                Ok(function.result.map(TypeSpec::kind))
            }
        }
    }

    fn for_loop(
        &mut self,
        line: usize,
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
        let step_value = range.step_by.as_ref().map_or(Some(1), literal_integer);
        if let (Some(from), Some(end), Some(step)) = (
            literal_integer(&range.from),
            literal_integer(end),
            step_value,
        ) && !range_can_end(from, end, step)
        {
            return Err(Problem::refused(
                line,
                "this loop's step can never reach its end (D74)",
            ));
        }
        if self.lookup(variable).is_some() {
            return Err(Problem::not_decided(
                line,
                format!(
                    "declaring `${variable}` again while the first one is still visible (shadowing) isn't decided"
                ),
            ));
        }
        self.scopes.push(HashMap::new());
        self.declare(variable, Role::LoopValue, ValueKind::Integer, line);
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
                body.first().map_or(0, |statement| statement.line),
                format!("a loop named `{name}` inside another loop named `{name}` isn't decided"),
            ));
        }
        self.loops.push(loop_name.cloned());
        let result = self.block(body);
        self.loops.pop();
        result
    }

    fn break_statement(&self, line: usize, loop_name: Option<&String>) -> Result<(), Problem> {
        if self.loops.is_empty() {
            return Err(Problem::refused(
                line,
                "`break` stops a loop, and there is no loop around it (D77)",
            ));
        }
        if let Some(name) = loop_name
            && !self.loops.iter().any(|other| other.as_ref() == Some(name))
        {
            return Err(Problem::refused(
                line,
                format!("there is no loop named `{name}` around this `break` (D77)"),
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
                condition.line,
                format!("{what} needs a `bool`; numbers never count as true or false (D60)"),
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
                expression.line,
                format!("{what} needs an integer"),
            ))
        }
    }

    fn expression(&self, expression: &Expression, context: Context) -> Result<ValueKind, Problem> {
        let line = expression.line;
        match &expression.kind {
            ExpressionKind::Integer(_) => Ok(ValueKind::Integer),
            ExpressionKind::Bool(_) => Ok(ValueKind::Bool),
            ExpressionKind::Text(_) | ExpressionKind::Newline => Err(Problem::not_decided(
                line,
                "text types aren't designed yet, so text can only be printed",
            )),
            ExpressionKind::Value(name) => self
                .lookup(name)
                .map(|known| known.kind)
                .ok_or_else(|| Problem::refused(line, format!("`${name}` isn't declared (D31)"))),
            ExpressionKind::Old(name) => {
                if context != Context::Ensures {
                    return Err(Problem::refused(
                        line,
                        "`old[…]` only means something in an `ensures` (D53)",
                    ));
                }
                match self.lookup(name) {
                    Some(known) if matches!(known.role, Role::Parameter(_)) => Ok(known.kind),
                    _ => Err(Problem::refused(
                        line,
                        format!("`old[…]` takes a parameter, and `${name}` isn't one (D53)"),
                    )),
                }
            }
            ExpressionKind::Result => {
                let result = self.function.and_then(|function| function.result);
                match (context, result) {
                    (Context::Ensures, Some(result)) => Ok(result.kind()),
                    _ => Err(Problem::refused(
                        line,
                        "`result` only means something in an `ensures` of a function with `->` (D53)",
                    )),
                }
            }
            ExpressionKind::Call(call) => self.call(call, context)?.ok_or_else(|| {
                Problem::refused(
                    line,
                    "this function has `-> void`, so it gives nothing back to use as a value (D99)",
                )
            }),
            ExpressionKind::Negate(inner) => self.operands(
                line,
                context,
                &[inner],
                ValueKind::Integer,
                "`-`",
                ValueKind::Integer,
            ),
            ExpressionKind::Not(inner) => self.operands(
                line,
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
            } => self.binary(line, context, *operator, left, right),
        }
    }

    fn binary(
        &self,
        line: usize,
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
                line,
                context,
                &sides,
                ValueKind::Integer,
                "arithmetic",
                ValueKind::Integer,
            ),
            Operator::Less | Operator::Greater | Operator::AtMost | Operator::AtLeast => self
                .operands(
                    line,
                    context,
                    &sides,
                    ValueKind::Integer,
                    "an ordering comparison",
                    ValueKind::Bool,
                ),
            Operator::And | Operator::Or => self.operands(
                line,
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
                        line,
                        "`equal` and `not_equal` compare two values of the same kind",
                    ))
                }
            }
        }
    }

    fn operands(
        &self,
        line: usize,
        context: Context,
        operands: &[&Expression],
        wanted: ValueKind,
        what: &str,
        gives: ValueKind,
    ) -> Result<ValueKind, Problem> {
        for operand in operands {
            if self.expression(operand, context)? != wanted {
                return Err(Problem::refused(
                    line,
                    format!("{what} needs {}", wanted.describe()),
                ));
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

    fn declare(&mut self, name: &str, role: Role, kind: ValueKind, line: usize) {
        let known = Known {
            role,
            kind,
            line,
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
        unchanged.sort_by_key(|(_, known)| known.line);
        if let Some((name, known)) = unchanged.first() {
            return Err(Problem::refused(
                known.line,
                format!("`${name}` is `var.mut`, but nothing changes it (D30)"),
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
                    labels.line,
                    format!(
                        "`labels` names `${}`, which isn't a parameter (D54)",
                        entry.parameter
                    ),
                ));
            }
            let earlier = labels.entries.iter().take(index);
            if earlier
                .clone()
                .any(|other| other.parameter == entry.parameter)
            {
                return Err(Problem::refused(
                    labels.line,
                    format!("`${}` is labelled twice (D54)", entry.parameter),
                ));
            }
            if earlier.clone().any(|other| other.label == entry.label) {
                return Err(Problem::refused(
                    labels.line,
                    format!("two parameters share the label `{}` (D54)", entry.label),
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
                default.line,
                format!(
                    "only labelled parameters may have defaults, and `${}` has no label (D54)",
                    default.parameter
                ),
            ));
        }
        if function
            .defaults
            .iter()
            .take(index)
            .any(|other| other.parameter == default.parameter)
        {
            return Err(Problem::refused(
                default.line,
                format!("`${}` has two defaults", default.parameter),
            ));
        }
        let Some(kind) = literal_kind(&default.value) else {
            return Err(Problem::not_decided(
                default.line,
                "defaults other than a plain number, `true` or `false` aren't decided",
            ));
        };
        let parameter = function
            .parameters
            .iter()
            .find(|parameter| parameter.name == default.parameter);
        if parameter.is_some_and(|parameter| parameter.type_spec.kind() != kind) {
            return Err(Problem::refused(
                default.line,
                format!(
                    "the default for `${}` is {}, which its type doesn't hold",
                    default.parameter,
                    kind.describe()
                ),
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

fn check_type(type_spec: TypeSpec, line: usize) -> Result<(), Problem> {
    if let TypeSpec::Integer(bounds) = type_spec
        && bounds.min > bounds.max
    {
        return Err(Problem::refused(
            line,
            format!("`min: {}` is above `max: {}` (D14)", bounds.min, bounds.max),
        ));
    }
    Ok(())
}

fn literal_integer(expression: &Expression) -> Option<i128> {
    match &expression.kind {
        ExpressionKind::Integer(number) => Some(*number),
        ExpressionKind::Negate(inner) => literal_integer(inner)?.checked_neg(),
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
