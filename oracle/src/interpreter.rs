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
use crate::rules::range_can_end;

/// The oracle stops after this many steps, so a program that never ends can't hang it.
const STEP_LIMIT: u64 = 10_000_000;

/// The oracle stops when calls nest deeper than this. The real limit is the target's proven stack (D8).
const DEPTH_LIMIT: usize = 200;

/// Why a run stopped before the end of `main`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Stop {
    /// A check site failed.
    CheckFails { line: usize, message: String },
    /// The oracle reached one of its own limits.
    GaveUp { line: usize, message: String },
    /// The oracle contradicted its own checker.
    Fault(String),
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
        .invoke(&program.main, frame, program.main.line)
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
}

/// How a statement or block ended.
enum Flow {
    Normal,
    /// A `break`, and the loop name it named.
    Break(Option<String>),
    /// A `return`, its value (`None` for `return void;`, D100), and its line (D95).
    Return(Option<Value>, usize),
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
            self.tick(statement.line)?;
            let flow = self.statement(statement, frame)?;
            if !matches!(flow, Flow::Normal) {
                return Ok(flow);
            }
        }
        Ok(Flow::Normal)
    }

    fn tick(&mut self, line: usize) -> Result<(), Stop> {
        self.steps = self.steps.saturating_add(1);
        if self.steps > STEP_LIMIT {
            return Err(Stop::GaveUp {
                line,
                message: format!("the oracle stopped after {STEP_LIMIT} steps"),
            });
        }
        Ok(())
    }

    fn statement(
        &mut self,
        statement: &'program Statement,
        frame: &mut Frame,
    ) -> Result<Flow, Stop> {
        let line = statement.line;
        match &statement.kind {
            StatementKind::Declare {
                name,
                type_spec,
                value,
                ..
            } => {
                let value = self.evaluate(value, frame)?;
                let bounds = type_spec.as_ref().and_then(TypeSpec::bounds).cloned();
                check_bounds(line, name, &value, bounds.as_ref())?;
                frame.declare(name, Slot { value, bounds });
                Ok(Flow::Normal)
            }
            StatementKind::Assign { name, value } => {
                let value = self.evaluate(value, frame)?;
                let slot = frame
                    .lookup_mut(name)
                    .ok_or_else(|| fault(format!("`${name}` has no slot")))?;
                check_bounds(line, name, &value, slot.bounds.as_ref())?;
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
            } => self.for_loop(line, loop_name.as_ref(), variable, range, body, frame),
            StatementKind::While {
                condition,
                decreases,
                body,
            } => self.while_loop(line, condition, decreases.as_ref(), body, frame),
            StatementKind::Forever { loop_name, body } => loop {
                self.tick(line)?;
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
                Ok(Flow::Return(value, line))
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
            return Err(Stop::CheckFails {
                line,
                message: "this loop's step can never reach its end (D74)".to_owned(),
            });
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
            self.tick(line)?;
            frame.scopes.push(HashMap::from([(
                variable.to_owned(),
                Slot {
                    value: Value::Integer(current.clone()),
                    bounds: None,
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
        line: usize,
        condition: &'program Expression,
        decreases: Option<&'program Expression>,
        body: &'program [Statement],
        frame: &mut Frame,
    ) -> Result<Flow, Stop> {
        let mut previous: Option<BigInt> = None;
        loop {
            self.tick(line)?;
            if !self.truth(condition, frame)? {
                return Ok(Flow::Normal);
            }
            if let Some(measure) = decreases {
                // trace D75: the measure shrinks every trip and never goes below zero.
                let value = self.integer(measure, frame)?;
                if value.sign() == Sign::Minus {
                    return Err(Stop::CheckFails {
                        line: measure.line,
                        message: format!("`decreases` went below zero, to {value} (D75)"),
                    });
                }
                if let Some(before) = &previous
                    && value >= *before
                {
                    return Err(Stop::CheckFails {
                        line: measure.line,
                        message: format!(
                            "`decreases` didn't shrink: {before} on one trip, {value} on the next (D75)"
                        ),
                    });
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
        let bindings = bind_arguments(function, call).map_err(|problem| fault(problem.message))?;
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
            check_bounds(
                call.line,
                &parameter.name,
                &value,
                parameter.type_spec.bounds(),
            )?;
            callee.old.insert(parameter.name.clone(), value.clone());
            callee.declare(
                &parameter.name,
                Slot {
                    value,
                    bounds: parameter.type_spec.bounds().cloned(),
                },
            );
        }
        self.invoke(function, callee, call.line)
    }

    /// Runs a function whose parameters are already in `callee`: `requires`, the body, the result, `ensures`.
    fn invoke(
        &mut self,
        function: &'program Function,
        mut callee: Frame,
        call_line: usize,
    ) -> Result<Option<Value>, Stop> {
        let name = &function.name;
        for condition in &function.requires {
            if !self.truth(condition, &mut callee)? {
                return Err(Stop::CheckFails {
                    line: call_line,
                    message: format!(
                        "this call doesn't meet `{name}`'s `requires` on line {} (D53)",
                        condition.line
                    ),
                });
            }
        }
        self.depth = self.depth.saturating_add(1);
        if self.depth > DEPTH_LIMIT {
            return Err(Stop::GaveUp {
                line: call_line,
                message: format!("calls nested deeper than the oracle's {DEPTH_LIMIT}"),
            });
        }
        let flow = self.block(&function.body, &mut callee);
        self.depth = self.depth.saturating_sub(1);
        match (flow?, function.result.as_ref()) {
            (Flow::Return(Some(value), line), Some(result)) => {
                // trace D95: the value given back is checked against the result type's bounds (D14).
                if let (Value::Integer(number), Some(bounds)) = (&value, result.bounds())
                    && (*number < bounds.min || *number > bounds.max)
                {
                    return Err(Stop::CheckFails {
                        line,
                        message: format!(
                            "`{name}` would give back {number}, outside int[min: {}, max: {}] (D14, D95)",
                            bounds.min, bounds.max
                        ),
                    });
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
                return Err(Stop::CheckFails {
                    line: condition.line,
                    message: format!("`{name}` doesn't keep this `ensures` (D53)"),
                });
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
        let line = expression.line;
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
                    arithmetic(line, *operator, &left, &right)
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

fn check_bounds(
    line: usize,
    name: &str,
    value: &Value,
    bounds: Option<&Bounds>,
) -> Result<(), Stop> {
    if let (Value::Integer(number), Some(bounds)) = (value, bounds)
        && (*number < bounds.min || *number > bounds.max)
    {
        return Err(Stop::CheckFails {
            line,
            message: format!(
                "`${name}` would hold {number}, outside int[min: {}, max: {}] (D14)",
                bounds.min, bounds.max
            ),
        });
    }
    Ok(())
}

fn arithmetic(
    line: usize,
    operator: Operator,
    left: &BigInt,
    right: &BigInt,
) -> Result<Value, Stop> {
    match operator {
        Operator::Add => Ok(Value::Integer(left + right)),
        Operator::Subtract => Ok(Value::Integer(left - right)),
        Operator::Multiply => Ok(Value::Integer(left * right)),
        Operator::Divide(rounding) => divide(line, rounding, left, right).map(Value::Integer),
        Operator::Remainder(rule) => remainder(line, rule, left, right).map(Value::Integer),
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
fn divide(line: usize, rounding: Rounding, left: &BigInt, right: &BigInt) -> Result<BigInt, Stop> {
    if right.sign() == Sign::NoSign {
        return Err(Stop::CheckFails {
            line,
            message: "division by zero (D37)".to_owned(),
        });
    }
    let quotient = left / right;
    let rest = left % right;
    let has_rest = rest.sign() != Sign::NoSign;
    let signs_differ = (rest.sign() == Sign::Minus) != (right.sign() == Sign::Minus);
    match rounding {
        Rounding::Exact if !has_rest => Ok(quotient),
        Rounding::Exact => Err(Stop::CheckFails {
            line,
            message: format!(
                "`div` is exact, but {left} div {right} leaves a remainder; write `div.floor`, `div.trunc` or `div.ceil` (D37)"
            ),
        }),
        Rounding::Floor if has_rest && signs_differ => Ok(quotient - 1),
        Rounding::Ceil if has_rest && !signs_differ => Ok(quotient + 1),
        Rounding::Trunc | Rounding::Floor | Rounding::Ceil => Ok(quotient),
    }
}

fn remainder(
    line: usize,
    rule: RemainderRule,
    left: &BigInt,
    right: &BigInt,
) -> Result<BigInt, Stop> {
    if right.sign() == Sign::NoSign {
        return Err(Stop::CheckFails {
            line,
            message: "remainder of a division by zero (D38)".to_owned(),
        });
    }
    let rest = left % right;
    match rule {
        RemainderRule::NonNegative if left.sign() == Sign::Minus || right.sign() == Sign::Minus => {
            Err(Stop::CheckFails {
                line,
                message: format!(
                    "bare `mod` needs both numbers non-negative, but this is {left} mod {right}; write `mod.floor`, `mod.trunc` or `mod.euclid` (D38)"
                ),
            })
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
