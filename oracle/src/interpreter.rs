// section oracle.interpreter:
//! Runs a checked program with every check site checked as it runs, as in mode 2 (D9): bounds (D14), exact `div`
//! (D37), bare `mod` (D38), contracts (D53), `decreases` (D75), ranges (D74) and results (D95).

use std::collections::HashMap;

use crate::ast::{
    Bounds, Call, CallTarget, Expression, ExpressionKind, Function, LoopRange, Operator, Program,
    RangeEnd, RemainderRule, Rounding, Statement, StatementKind, TypeSpec,
};
use num_bigint::{BigInt, Sign};

use crate::binding::{Binding, bind_arguments};
use crate::outcome::{Place, Problem};
use crate::rules::range_can_end;

/// The oracle stops after this many steps, so a program that never ends can't hang it.
const STEP_LIMIT: u64 = 10_000_000;

/// The oracle stops when calls nest deeper than this. The real limit is the target's proven stack (D8).
const DEPTH_LIMIT: usize = 200;

/// Why a run stopped before the end of `fn.main`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Stop {
    /// A check failed (D114), or the oracle reached one of its own limits.
    Problem(Problem),
    /// The oracle contradicted its own checker.
    Fault(String),
}

impl From<Problem> for Stop {
    fn from(problem: Problem) -> Self {
        Self::Problem(problem)
    }
}

/// Runs `fn.main` (D101), returning how it ended and everything printed.
pub(crate) fn run(program: &Program) -> (Result<(), Stop>, String) {
    let mut machine = Machine {
        functions: program
            .functions
            .iter()
            .map(|function| (function.name.as_str(), function))
            .collect(),
        output: String::new(),
        steps: 0,
        depth: 0,
    };
    let frame = Frame {
        scopes: vec![HashMap::new()],
        old: HashMap::new(),
        result: None,
    };
    let result = machine
        .invoke(&program.main, frame, program.main.place)
        .map(|_| ());
    (result, machine.output)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Value {
    /// Any size, so the oracle never runs out of digits before Radium does (D104).
    Integer(BigInt),
    Bool(bool),
}

#[derive(Debug, Clone)]
struct Slot {
    value: Value,
    bounds: Option<Bounds>,
    /// Where the value's bounds are written, for `because:` links (D122).
    declared: Place,
}

/// How a statement or block ended.
enum Flow {
    Normal,
    /// A `break`, and the loop name it named.
    Break(Option<String>),
    /// A `return`, its value (`None` for `return void;`, D100), and its place (D95).
    Return(Option<Value>, Place),
}

/// One function call's values.
struct Frame {
    scopes: Vec<HashMap<String, Slot>>,
    /// Each parameter's value when the call started, for `old[…]` (D53).
    old: HashMap<String, Value>,
    /// The value given back, for `result` in an `ensures` (D53).
    result: Option<Value>,
}

impl Frame {
    fn declare(&mut self, name: &str, slot: Slot) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_owned(), slot);
        }
    }

    fn lookup(&self, name: &str) -> Option<&Slot> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut Slot> {
        self.scopes
            .iter_mut()
            .rev()
            .find_map(|scope| scope.get_mut(name))
    }
}

struct Machine<'program> {
    functions: HashMap<&'program str, &'program Function>,
    output: String,
    steps: u64,
    depth: usize,
}

impl<'program> Machine<'program> {
    fn block(
        &mut self,
        statements: &'program [Statement],
        frame: &mut Frame,
    ) -> Result<Flow, Stop> {
        frame.scopes.push(HashMap::new());
        let result = self.statements(statements, frame);
        frame.scopes.pop();
        result
    }

    fn statements(
        &mut self,
        statements: &'program [Statement],
        frame: &mut Frame,
    ) -> Result<Flow, Stop> {
        for statement in statements {
            self.tick(statement.place)?;
            let flow = self.statement(statement, frame)?;
            if !matches!(flow, Flow::Normal) {
                return Ok(flow);
            }
        }
        Ok(Flow::Normal)
    }

    fn tick(&mut self, place: Place) -> Result<(), Stop> {
        self.steps = self.steps.saturating_add(1);
        if self.steps > STEP_LIMIT {
            return Err(Problem::gave_up(
                place,
                format!("the oracle stopped after {STEP_LIMIT} steps"),
            )
            .into());
        }
        Ok(())
    }

    fn statement(
        &mut self,
        statement: &'program Statement,
        frame: &mut Frame,
    ) -> Result<Flow, Stop> {
        let place = statement.place;
        match &statement.kind {
            StatementKind::Declare {
                name,
                type_spec,
                value: expression,
                ..
            } => {
                let value = self.evaluate(expression, frame)?;
                let bounds = type_spec.as_ref().and_then(TypeSpec::bounds).cloned();
                let slot = Slot {
                    value,
                    bounds,
                    declared: place,
                };
                check_bounds(expression.place, name, &slot.value, &slot)?;
                frame.declare(name, slot);
                Ok(Flow::Normal)
            }
            StatementKind::Assign {
                name,
                value: expression,
            } => {
                let value = self.evaluate(expression, frame)?;
                let slot = frame
                    .lookup_mut(name)
                    .ok_or_else(|| fault(format!("`${name}` has no slot")))?;
                check_bounds(expression.place, name, &value, slot)?;
                slot.value = value;
                Ok(Flow::Normal)
            }
            StatementKind::Call(call) => {
                self.call(call, frame)?;
                Ok(Flow::Normal)
            }
            StatementKind::If {
                branches,
                else_body,
            } => {
                for branch in branches {
                    if self.truth(&branch.condition, frame)? {
                        return self.block(&branch.body, frame);
                    }
                }
                match else_body {
                    Some(body) => self.block(body, frame),
                    None => Ok(Flow::Normal),
                }
            }
            StatementKind::For {
                loop_name,
                variable,
                range,
                body,
            } => self.for_loop(place, loop_name.as_ref(), variable, range, body, frame),
            StatementKind::While {
                condition,
                decreases,
                body,
            } => self.while_loop(place, condition, decreases.as_ref(), body, frame),
            StatementKind::Forever { loop_name, body } => loop {
                self.tick(place)?;
                if let Some(flow) = after_trip(self.block(body, frame)?, loop_name.as_ref()) {
                    return Ok(flow);
                }
            },
            StatementKind::Break { loop_name } => Ok(Flow::Break(loop_name.clone())),
            StatementKind::Return { value } => {
                let value = match value {
                    Some(value) => Some(self.evaluate(value, frame)?),
                    None => None,
                };
                Ok(Flow::Return(value, place))
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
        frame: &mut Frame,
    ) -> Result<Flow, Stop> {
        let from = self.integer(&range.from, frame)?;
        let (end, inclusive) = match &range.end {
            RangeEnd::To(end) => (self.integer(end, frame)?, true),
            RangeEnd::Before(end) => (self.integer(end, frame)?, false),
        };
        let step = match &range.step_by {
            Some(step) => self.integer(step, frame)?,
            None => BigInt::from(1),
        };
        if !range_can_end(&from, &end, &step) {
            let end_label = if inclusive { "to" } else { "before" };
            return Err(Problem::check_failed(
                place,
                "this loop's step can never reach its end",
                "a loop's step must lead from `from:` to its end (D74)",
            )
            .when(format!("from: {from}, {end_label}: {end}, step_by: {step}"))
            .into());
        }
        let mut current = from;
        loop {
            let inside = match (step.sign() == Sign::Plus, inclusive) {
                (true, true) => current <= end,
                (true, false) => current < end,
                (false, true) => current >= end,
                (false, false) => current > end,
            };
            if !inside {
                return Ok(Flow::Normal);
            }
            self.tick(place)?;
            frame.scopes.push(HashMap::from([(
                variable.to_owned(),
                Slot {
                    value: Value::Integer(current.clone()),
                    bounds: None,
                    declared: place,
                },
            )]));
            let flow = self.block(body, frame);
            frame.scopes.pop();
            if let Some(flow) = after_trip(flow?, loop_name) {
                return Ok(flow);
            }
            current += &step;
        }
    }

    fn while_loop(
        &mut self,
        place: Place,
        condition: &'program Expression,
        decreases: Option<&'program Expression>,
        body: &'program [Statement],
        frame: &mut Frame,
    ) -> Result<Flow, Stop> {
        let mut previous: Option<BigInt> = None;
        loop {
            self.tick(place)?;
            if !self.truth(condition, frame)? {
                return Ok(Flow::Normal);
            }
            if let Some(measure) = decreases {
                // trace D75: the measure shrinks every trip and never goes below zero.
                let value = self.integer(measure, frame)?;
                if value.sign() == Sign::Minus {
                    return Err(Problem::check_failed(
                        measure.place,
                        "`decreases` went below zero",
                        "a `decreases` value shrinks every trip and never goes below zero (D75)",
                    )
                    .when(format!("it is {value}"))
                    .into());
                }
                if let Some(before) = &previous
                    && value >= *before
                {
                    return Err(Problem::check_failed(
                        measure.place,
                        "`decreases` didn't shrink",
                        "a `decreases` value shrinks every trip and never goes below zero (D75)",
                    )
                    .when(format!("{before} on one trip, {value} on the next"))
                    .into());
                }
                previous = Some(value);
            }
            if let Some(flow) = after_trip(self.block(body, frame)?, None) {
                return Ok(flow);
            }
        }
    }

    /// Runs a call, giving back its result, if the function has one.
    fn call(&mut self, call: &'program Call, frame: &mut Frame) -> Result<Option<Value>, Stop> {
        let CallTarget::Own { name, .. } = &call.target else {
            self.print(call, frame)?;
            return Ok(None);
        };
        let function = *self
            .functions
            .get(name.as_str())
            .ok_or_else(|| fault(format!("no function `{name}`")))?;
        let bindings = bind_arguments(function, call).map_err(|problem| fault(problem.what()))?;
        // trace D55: arguments are evaluated left to right, as written.
        let mut argument_values = Vec::with_capacity(call.arguments.len());
        for argument in &call.arguments {
            argument_values.push(self.evaluate(&argument.value, frame)?);
        }
        let mut callee = Frame {
            scopes: vec![HashMap::new()],
            old: HashMap::new(),
            result: None,
        };
        for (parameter, binding) in function.parameters.iter().zip(bindings) {
            let value = match binding {
                Binding::Argument(index) => argument_values
                    .get(index)
                    .cloned()
                    .ok_or_else(|| fault("an argument went missing"))?,
                Binding::Default(index) => {
                    let default = function
                        .defaults
                        .get(index)
                        .ok_or_else(|| fault("a default went missing"))?;
                    self.evaluate(&default.value, &mut callee)?
                }
            };
            let slot = Slot {
                value,
                bounds: parameter.type_spec.bounds().cloned(),
                declared: parameter.place,
            };
            let argument_place = match binding {
                Binding::Argument(index) => call
                    .arguments
                    .get(index)
                    .map_or(call.place, |argument| argument.value.place),
                Binding::Default(_) => call.place,
            };
            check_bounds(argument_place, &parameter.name, &slot.value, &slot)?;
            callee
                .old
                .insert(parameter.name.clone(), slot.value.clone());
            callee.declare(&parameter.name, slot);
        }
        self.invoke(function, callee, call.place)
    }

    /// Runs a function whose parameters are already in `callee`: `requires`, the body, the result, `ensures`.
    fn invoke(
        &mut self,
        function: &'program Function,
        mut callee: Frame,
        call_place: Place,
    ) -> Result<Option<Value>, Stop> {
        let name = &function.name;
        for condition in &function.requires {
            if !self.truth(condition, &mut callee)? {
                return Err(Problem::check_failed(
                    call_place,
                    format!("this call doesn't meet `{name}`'s `requires`"),
                    "callers must guarantee what a function `requires` (D53)",
                )
                .because(condition.place, format!("`{name}` requires this"))
                .into());
            }
        }
        self.depth = self.depth.saturating_add(1);
        if self.depth > DEPTH_LIMIT {
            return Err(Problem::gave_up(
                call_place,
                format!("calls nested deeper than the oracle's {DEPTH_LIMIT}"),
            )
            .into());
        }
        let flow = self.block(&function.body, &mut callee);
        self.depth = self.depth.saturating_sub(1);
        match (flow?, function.result.as_ref()) {
            (Flow::Return(Some(value), place), Some(result)) => {
                // trace D95: the value given back is checked against the result type's bounds (D14).
                if let (Value::Integer(number), Some(bounds)) = (&value, result.bounds())
                    && (*number < bounds.min || *number > bounds.max)
                {
                    return Err(Problem::check_failed(
                        place,
                        format!(
                            "`{name}` gives back a value outside int[min: {}, max: {}]",
                            bounds.min, bounds.max
                        ),
                        "a function's result stays within its result type's bounds (D14, D95)",
                    )
                    .when(format!("it would give back {number}"))
                    .because(
                        function.place,
                        format!("`{name}`'s result type is set here"),
                    )
                    .into());
                }
                callee.result = Some(value);
            }
            (Flow::Return(None, _), None) => {}
            (Flow::Normal, _) => return Err(fault("a function reached its end without `return`")),
            (Flow::Return(..), _) => {
                return Err(fault("a `return` didn't match its function's `->`"));
            }
            (Flow::Break(_), _) => return Err(fault("a `break` escaped a function")),
        }
        for condition in &function.ensures {
            if !self.truth(condition, &mut callee)? {
                let mut problem = Problem::check_failed(
                    condition.place,
                    format!("`{name}` doesn't keep this `ensures`"),
                    "a function keeps every promise its `ensures` makes (D53)",
                );
                if let Some(result) = &callee.result {
                    problem = problem.when(format!("`result` is {}", show(result)));
                }
                return Err(problem.into());
            }
        }
        Ok(callee.result)
    }

    fn print(&mut self, call: &'program Call, frame: &mut Frame) -> Result<(), Stop> {
        for argument in &call.arguments {
            match &argument.value.kind {
                ExpressionKind::Text(text) => self.output.push_str(text),
                ExpressionKind::Newline => self.output.push('\n'),
                _ => match self.evaluate(&argument.value, frame)? {
                    Value::Integer(number) => self.output.push_str(&number.to_string()),
                    Value::Bool(truth) => {
                        self.output.push_str(if truth { "true" } else { "false" });
                    }
                },
            }
        }
        Ok(())
    }

    fn truth(&mut self, expression: &'program Expression, frame: &mut Frame) -> Result<bool, Stop> {
        match self.evaluate(expression, frame)? {
            Value::Bool(truth) => Ok(truth),
            Value::Integer(_) => Err(fault("an integer where the checker promised a `bool`")),
        }
    }

    fn integer(
        &mut self,
        expression: &'program Expression,
        frame: &mut Frame,
    ) -> Result<BigInt, Stop> {
        match self.evaluate(expression, frame)? {
            Value::Integer(number) => Ok(number),
            Value::Bool(_) => Err(fault("a `bool` where the checker promised an integer")),
        }
    }

    fn evaluate(
        &mut self,
        expression: &'program Expression,
        frame: &mut Frame,
    ) -> Result<Value, Stop> {
        let place = expression.place;
        match &expression.kind {
            ExpressionKind::Integer(number) => Ok(Value::Integer(number.clone())),
            ExpressionKind::Bool(truth) => Ok(Value::Bool(*truth)),
            ExpressionKind::Text(_) | ExpressionKind::Newline => {
                Err(fault("text outside `std::print`"))
            }
            ExpressionKind::Value(name) => frame
                .lookup(name)
                .map(|slot| slot.value.clone())
                .ok_or_else(|| fault(format!("`${name}` has no slot"))),
            ExpressionKind::Old(name) => frame
                .old
                .get(name)
                .cloned()
                .ok_or_else(|| fault(format!("`old[${name}]` has no value"))),
            ExpressionKind::Result => frame
                .result
                .clone()
                .ok_or_else(|| fault("`result` before anything was given back")),
            ExpressionKind::Call(call) => self
                .call(call, frame)?
                .ok_or_else(|| fault("a call used as a value gave nothing back")),
            ExpressionKind::Negate(inner) => Ok(Value::Integer(-self.integer(inner, frame)?)),
            ExpressionKind::Not(inner) => Ok(Value::Bool(!self.truth(inner, frame)?)),
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } => match operator {
                // trace D40: `and` and `or` short-circuit.
                Operator::And => Ok(Value::Bool(
                    self.truth(left, frame)? && self.truth(right, frame)?,
                )),
                Operator::Or => Ok(Value::Bool(
                    self.truth(left, frame)? || self.truth(right, frame)?,
                )),
                Operator::Equal | Operator::NotEqual => {
                    let same = self.evaluate(left, frame)? == self.evaluate(right, frame)?;
                    Ok(Value::Bool(same == (*operator == Operator::Equal)))
                }
                _ => {
                    let left = self.integer(left, frame)?;
                    let right = self.integer(right, frame)?;
                    arithmetic(place, *operator, &left, &right)
                }
            },
        }
    }
}

/// After one trip of a loop: `None` to go on, or how the loop ends. A `return` passes straight through.
fn after_trip(flow: Flow, loop_name: Option<&String>) -> Option<Flow> {
    match flow {
        Flow::Normal => None,
        Flow::Break(target) => Some(finish_loop(target, loop_name)),
        Flow::Return(value, line) => Some(Flow::Return(value, line)),
    }
}

/// After a `break` reaches a loop: the loop stops if the `break` was bare or named it; otherwise the `break` goes on
/// outward (D77).
fn finish_loop(target: Option<String>, loop_name: Option<&String>) -> Flow {
    if target.is_none() || target.as_ref() == loop_name {
        Flow::Normal
    } else {
        Flow::Break(target)
    }
}

/// Checks a value against the bounds of the slot it is going into (D14).
fn check_bounds(place: Place, name: &str, value: &Value, slot: &Slot) -> Result<(), Stop> {
    if let (Value::Integer(number), Some(bounds)) = (value, &slot.bounds)
        && (*number < bounds.min || *number > bounds.max)
    {
        return Err(Problem::check_failed(
            place,
            format!(
                "`${name}` holds int[min: {}, max: {}], and this value doesn't fit",
                bounds.min, bounds.max
            ),
            "every value stays within its bounds (D14)",
        )
        .when(format!("it would be {number}"))
        .because(slot.declared, format!("`${name}`'s bounds are set here"))
        .into());
    }
    Ok(())
}

/// A value as `std::print` would show it.
fn show(value: &Value) -> String {
    match value {
        Value::Integer(number) => number.to_string(),
        Value::Bool(truth) => truth.to_string(),
    }
}

fn arithmetic(
    place: Place,
    operator: Operator,
    left: &BigInt,
    right: &BigInt,
) -> Result<Value, Stop> {
    match operator {
        Operator::Add => Ok(Value::Integer(left + right)),
        Operator::Subtract => Ok(Value::Integer(left - right)),
        Operator::Multiply => Ok(Value::Integer(left * right)),
        Operator::Divide(rounding) => divide(place, rounding, left, right).map(Value::Integer),
        Operator::Remainder(rule) => remainder(place, rule, left, right).map(Value::Integer),
        Operator::Less => Ok(Value::Bool(left < right)),
        Operator::Greater => Ok(Value::Bool(left > right)),
        Operator::AtMost => Ok(Value::Bool(left <= right)),
        Operator::AtLeast => Ok(Value::Bool(left >= right)),
        Operator::Equal | Operator::NotEqual | Operator::And | Operator::Or => {
            Err(fault("a comparison or logic operator reached arithmetic"))
        }
    }
}

/// `/` and `%` on big integers truncate toward zero, and the rest takes the sign of `left`.
fn divide(place: Place, rounding: Rounding, left: &BigInt, right: &BigInt) -> Result<BigInt, Stop> {
    if right.sign() == Sign::NoSign {
        return Err(Problem::check_failed(
            place,
            "a division by zero",
            "nothing is divided by zero (D37)",
        )
        .when(format!("{left} div 0"))
        .into());
    }
    let quotient = left / right;
    let rest = left % right;
    let has_rest = rest.sign() != Sign::NoSign;
    let signs_differ = (rest.sign() == Sign::Minus) != (right.sign() == Sign::Minus);
    match rounding {
        Rounding::Exact if !has_rest => Ok(quotient),
        Rounding::Exact => Err(Problem::check_failed(
            place,
            "this division leaves a remainder",
            "bare `div` is exact; rounding is only ever written, as a narrowing (D37)",
        )
        .when(format!("{left} div {right}"))
        .into()),
        Rounding::Floor if has_rest && signs_differ => Ok(quotient - 1),
        Rounding::Ceil if has_rest && !signs_differ => Ok(quotient + 1),
        Rounding::Trunc | Rounding::Floor | Rounding::Ceil => Ok(quotient),
    }
}

fn remainder(
    place: Place,
    rule: RemainderRule,
    left: &BigInt,
    right: &BigInt,
) -> Result<BigInt, Stop> {
    if right.sign() == Sign::NoSign {
        return Err(Problem::check_failed(
            place,
            "a remainder of a division by zero",
            "nothing is divided by zero (D38)",
        )
        .when(format!("{left} mod 0"))
        .into());
    }
    let rest = left % right;
    match rule {
        RemainderRule::NonNegative if left.sign() == Sign::Minus || right.sign() == Sign::Minus => {
            Err(Problem::check_failed(
                place,
                "bare `mod` with a negative number",
                "bare `mod` needs both numbers non-negative, where every definition agrees (D38)",
            )
            .when(format!("{left} mod {right}"))
            .into())
        }
        RemainderRule::Floor
            if rest.sign() != Sign::NoSign
                && (rest.sign() == Sign::Minus) != (right.sign() == Sign::Minus) =>
        {
            Ok(rest + right)
        }
        RemainderRule::Euclid if rest.sign() == Sign::Minus => Ok(if right.sign() == Sign::Minus {
            rest - right
        } else {
            rest + right
        }),
        RemainderRule::NonNegative
        | RemainderRule::Trunc
        | RemainderRule::Floor
        | RemainderRule::Euclid => Ok(rest),
    }
}

fn fault(message: impl Into<String>) -> Stop {
    Stop::Fault(message.into())
}
// end section oracle.interpreter
