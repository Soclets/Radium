// section oracle.checker.borrows:
//! Stored borrows (D49, D125–D139): where each value's borrows come from, that a borrow goes only where its type names
//! its source (D130), how long sources last (D127, D132), and that a source doesn't change or go away while a borrow of
//! it is still used (D5, D129).

use std::collections::HashMap;

use num_bigint::BigInt;

use super::{Checker, Known, Role, undeclared};
use crate::ast::{
    Bounds, Call, CallTarget, Expression, ExpressionKind, FieldValue, Function, Handoff, Holding,
    LoopSource, RangeEnd, Source, SourceFill, Statement, StatementKind, Step, TypeSpec, ValueKind,
};
use crate::binding::{Binding, bind_arguments};
use crate::outcome::{Place, Problem};
use crate::types::{
    check_source_declarations, collect_sources, holds_a_borrow, lasts_by_rules, source_rules,
};

/// Where a type is written in a function, which decides what its sources may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Position {
    Parameter,
    Result,
    Local,
}

/// Why a value doesn't fit where it goes.
enum Misfit {
    /// It borrows from a source the holder's type doesn't name; the holder's sources there.
    Extra(Source, Vec<Source>),
    BorrowIntoOwned,
    OwnedIntoBorrow,
}

/// Where a value goes, for messages.
pub(super) enum Destination<'name> {
    Name(&'name str),
    Result,
    Field(&'name str),
}

impl Destination<'_> {
    fn describe(&self) -> String {
        match self {
            Self::Name(name) => format!("`${name}`"),
            Self::Result => "the result".to_owned(),
            Self::Field(name) => format!("the field `{name}`"),
        }
    }
}

// Written types: what their sources may name (D49, D126, D132, D136).
impl Checker<'_> {
    /// Checks a function's own sources, the sources its parameters' and result's types name, and its
    /// `lasts_at_least` lines (D49, D132, D133, D134, D136).
    pub(super) fn function_sources(&self, function: &Function) -> Result<(), Problem> {
        if function.is_main && !function.sources.is_empty() {
            return Err(Problem::not_decided(
                function.place,
                "named sources on `fn.main`, which nothing calls (D101, D136)",
            ));
        }
        check_source_declarations(&function.sources)?;
        for source in &function.sources {
            let used = function.parameters.iter().any(|parameter| {
                let mut named = Vec::new();
                collect_sources(&parameter.type_spec, &mut named);
                named.contains(&Source::Named(source.name.clone()))
            });
            if !used {
                return Err(Problem::refused(
                    source.place,
                    format!("no parameter's type uses `` `{}` ``", source.name),
                    "every source a function declares is used, here by a parameter's type (D133, D136)",
                ));
            }
        }
        for parameter in &function.parameters {
            self.written_type(&parameter.type_spec, parameter.place, Position::Parameter)?;
        }
        if let Some(result) = &function.result {
            self.written_type(result, function.place, Position::Result)?;
        }
        let mut facts: HashMap<(&str, &str), Place> = HashMap::new();
        for condition in &function.requires {
            let ExpressionKind::LastsAtLeast { longer, shorter } = &condition.kind else {
                continue;
            };
            for name in [longer, shorter] {
                let is_parameter = self
                    .lookup(name)
                    .is_some_and(|known| matches!(known.role, Role::Parameter { .. }));
                if !is_parameter {
                    return Err(Problem::refused(
                        condition.place,
                        format!("`${name}` isn't a parameter"),
                        "a `requires` line's `lasts_at_least` compares the function's parameters (D132)",
                    ));
                }
            }
            if longer == shorter {
                return Err(Problem::not_decided(
                    condition.place,
                    "a parameter said to last at least as long as itself",
                ));
            }
            if let Some(first) = facts.insert((longer.as_str(), shorter.as_str()), condition.place)
            {
                return Err(Problem::refused(
                    condition.place,
                    "the same `lasts_at_least` fact written twice",
                    "a fact is written once (D124, D127)",
                )
                .because(first, "it is written here first"));
            }
        }
        Ok(())
    }

    /// Checks the sources a type written in a function names, and that its structs' `lasts_at_least` rules hold.
    pub(super) fn written_type(
        &self,
        type_spec: &TypeSpec,
        place: Place,
        position: Position,
    ) -> Result<(), Problem> {
        if position == Position::Parameter && matches!(type_spec, TypeSpec::Borrow { .. }) {
            return Err(Problem::not_decided(
                place,
                "a parameter whose type is itself a stored borrow",
            ));
        }
        let mut sources = Vec::new();
        collect_sources(type_spec, &mut sources);
        for source in &sources {
            match source {
                Source::Value(name) => self.value_source(name, place, position)?,
                Source::Named(name) => {
                    let declared = self.function.is_some_and(|function| {
                        function.sources.iter().any(|source| &source.name == name)
                    });
                    if !declared {
                        return Err(Problem::refused(
                            place,
                            format!("there is no source `` `{name}` `` here"),
                            "a function's own sources are declared in its `< >` group (D136)",
                        ));
                    }
                }
                Source::Program => {}
            }
        }
        self.types.check_nested_rules(
            type_spec,
            place,
            &|longer, shorter| self.lasts(longer, shorter),
            &|longer, shorter| match (longer, shorter) {
                (Source::Value(longer), Source::Value(shorter))
                    if self.is_parameter(longer) && self.is_parameter(shorter) =>
                {
                    Some(format!("`requires ${longer} lasts_at_least ${shorter};`"))
                }
                _ => None,
            },
        )
    }

    /// Checks a `$name` used as a source.
    fn value_source(&self, name: &str, place: Place, position: Position) -> Result<(), Problem> {
        if position == Position::Parameter {
            return Err(Problem::refused(
                place,
                format!("a parameter's type names `${name}`"),
                "a borrow inside a parameter comes from the caller, so it is named by the function's own sources (D136)",
            ));
        }
        let Some(known) = self.lookup(name) else {
            if position == Position::Result {
                return Err(Problem::refused(
                    place,
                    format!("`${name}` isn't a parameter"),
                    "a result borrows from a parameter, a function's own source or `program` (D49, D127, D136)",
                ));
            }
            return Err(undeclared(place, name));
        };
        if known.kind.is_copied() {
            return Err(Problem::refused(
                place,
                format!("`${name}` is always copied, so nothing borrows from it"),
                "integers and `bool` are always copied, so they are never lent (D92)",
            )
            .because(known.place, format!("`${name}` is declared here")));
        }
        if position == Position::Result
            && matches!(
                known.role,
                Role::Parameter {
                    holding: Holding::Given,
                    ..
                }
            )
        {
            return Err(Problem::refused(
                place,
                format!("the result borrows from `${name}`, which is given to the function"),
                "a value given to a function ends when the function returns, so nothing returned can borrow from it (D80, D127)",
            )
            .because(known.place, format!("`${name}` is declared `Given` here")));
        }
        Ok(())
    }

    /// Whether `longer` lasts at least as long as `shorter`, where the function being checked is (D127, D132). A
    /// value lasts until the end of its block (D140), so:
    /// `program` lasts longest; a parameter outlasts every local name; of two visible locals, the one declared first
    /// lasts at least as long; two parameters compare only through `requires` lines; the function's own sources
    /// outlast its locals and compare by their own rules.
    pub(super) fn lasts(&self, longer: &Source, shorter: &Source) -> bool {
        // trace D140: every name ends with its block, so an earlier one in view lasts at least as long.
        if longer == shorter {
            return true;
        }
        match (longer, shorter) {
            (Source::Program, _) => true,
            (_, Source::Program) | (Source::Value(_), Source::Named(_)) => false,
            (Source::Named(_), Source::Named(_)) => {
                let rules = self
                    .function
                    .map(|function| source_rules(&function.sources))
                    .unwrap_or_default();
                lasts_by_rules(&rules, longer, shorter)
            }
            (Source::Named(_), Source::Value(name)) => !self.is_parameter(name),
            (Source::Value(longer), Source::Value(shorter)) => {
                match (self.is_parameter(longer), self.is_parameter(shorter)) {
                    (true, true) => self.required_to_last(longer, shorter),
                    (true, false) => true,
                    (false, true) => false,
                    (false, false) => match (self.lookup(longer), self.lookup(shorter)) {
                        (Some(long), Some(short)) => {
                            (long.place.line, long.place.byte_column)
                                <= (short.place.line, short.place.byte_column)
                        }
                        _ => false,
                    },
                }
            }
        }
    }

    fn is_parameter(&self, name: &str) -> bool {
        self.lookup(name)
            .is_some_and(|known| matches!(known.role, Role::Parameter { .. }))
    }

    /// Whether the function's `requires` lines say `$longer` lasts at least as long as `$shorter`, directly or
    /// through a chain (D132).
    fn required_to_last(&self, longer: &str, shorter: &str) -> bool {
        let Some(function) = self.function else {
            return false;
        };
        let rules: Vec<(String, String)> = function
            .requires
            .iter()
            .filter_map(|condition| match &condition.kind {
                ExpressionKind::LastsAtLeast { longer, shorter } => {
                    Some((longer.clone(), shorter.clone()))
                }
                _ => None,
            })
            .collect();
        lasts_by_rules(
            &rules,
            &Source::Named(longer.to_owned()),
            &Source::Named(shorter.to_owned()),
        )
    }
}

// The full type of a value: where its borrows come from.
impl Checker<'_> {
    /// The type of a name, with its borrows' sources, or its shape when only that is known.
    pub(super) fn type_of(known: &Known) -> TypeSpec {
        known
            .type_spec
            .clone()
            .unwrap_or_else(|| shape_spec(&known.kind))
    }

    /// The full type of a value, with where its borrows come from; `None` when it holds no borrow.
    pub(super) fn value_type(&self, value: &Expression) -> Result<Option<TypeSpec>, Problem> {
        match &value.kind {
            ExpressionKind::Handoff { handoff, name } => {
                let Some(known) = self.lookup(name) else {
                    return Ok(None);
                };
                let held = Self::type_of(known);
                Ok(match handoff {
                    // trace D142: lending a `Lend` borrows from what it borrows from.
                    Handoff::Lend | Handoff::LendMut => match held {
                        TypeSpec::Borrow { .. } => Some(held),
                        inner => Some(TypeSpec::Borrow {
                            exclusive: *handoff == Handoff::LendMut,
                            sources: vec![Source::Value(name.clone())],
                            inner: Box::new(inner),
                        }),
                    },
                    // trace D143: a copy of a borrow is a value of its own; a copy of a struct keeps its borrows.
                    Handoff::Copy => {
                        let owned = held.owned().clone();
                        holds_a_borrow(&owned).then_some(owned)
                    }
                    Handoff::Give => holds_a_borrow(&held).then_some(held),
                })
            }
            ExpressionKind::Build {
                type_name,
                case: None,
                fields,
            } if !self.types.sources(type_name).is_empty() => Ok(Some(TypeSpec::Named {
                name: type_name.clone(),
                fills: self.build_fills(type_name, fields)?,
            })),
            ExpressionKind::Call(call) => self.call_result(call),
            ExpressionKind::Array { items, .. } => {
                let mut merged: Option<TypeSpec> = None;
                for item in items {
                    if let Some(item_type) = self.value_type(item)? {
                        match &mut merged {
                            Some(merged) => merge(merged, &item_type),
                            None => merged = Some(item_type),
                        }
                    }
                }
                Ok(merged.map(|element| TypeSpec::Array {
                    element: Box::new(element),
                    length: BigInt::from(items.len()),
                }))
            }
            _ => Ok(None),
        }
    }

    /// The sources a struct value is built with, worked out from its fields, after checking each field's value fits
    /// its field (D126, D130).
    pub(super) fn build_fills(
        &self,
        type_name: &str,
        values: &[FieldValue],
    ) -> Result<Vec<SourceFill>, Problem> {
        let Some(definition) = self.types.structure(type_name) else {
            return Ok(Vec::new());
        };
        let mut bound: HashMap<String, Vec<Source>> = HashMap::new();
        let mut typed = Vec::new();
        for value in values {
            let Some(field) = definition
                .fields
                .iter()
                .find(|field| field.name == value.name)
            else {
                continue;
            };
            let value_type = self.value_type(&value.value)?;
            if let Some(value_type) = &value_type {
                bind_sources(&field.type_spec, value_type, &mut bound);
            }
            typed.push((value, field, value_type));
        }
        for (value, field, value_type) in typed {
            let wanted = substitute(&field.type_spec, &HashMap::new(), &bound);
            if let Some(misfit) = fits(value_type.as_ref(), &wanted) {
                return Err(self.misfit(misfit, &value.value, &Destination::Field(&value.name)));
            }
        }
        Ok(definition
            .sources
            .iter()
            .map(|source| SourceFill {
                name: source.name.clone(),
                sources: bound.remove(&source.name).unwrap_or_default(),
            })
            .collect())
    }

    /// The full type of a call's result: the function's result type with its parameters replaced by where the
    /// arguments come from (D49, D136). Checks the function's `lasts_at_least` lines against the arguments (D132), and
    /// refuses a borrow from a fresh value (D139).
    pub(super) fn call_result(&self, call: &Call) -> Result<Option<TypeSpec>, Problem> {
        let CallTarget::Own { name, .. } = &call.target else {
            return Ok(None);
        };
        let Some(function) = self.functions.get(name.as_str()).copied() else {
            return Ok(None);
        };
        let Some(result) = &function.result else {
            return Ok(None);
        };
        if !holds_a_borrow(result) && function.sources.is_empty() {
            return Ok(None);
        }
        let mut result_sources = Vec::new();
        collect_sources(result, &mut result_sources);
        let mut values: HashMap<String, Vec<Source>> = HashMap::new();
        let mut named: HashMap<String, Vec<Source>> = HashMap::new();
        for (parameter, binding) in function
            .parameters
            .iter()
            .zip(bind_arguments(function, call)?)
        {
            let argument = match binding {
                Binding::Argument(index) => call.arguments.get(index),
                Binding::Default(_) => None,
            };
            let lent =
                argument.and_then(|argument| match (&argument.handoff, &argument.value.kind) {
                    (Some(Handoff::Lend | Handoff::LendMut), ExpressionKind::Value(name)) => {
                        self.lookup(name).map(|known| (name, known))
                    }
                    _ => None,
                });
            let Some((argument_name, known)) = lent else {
                if result_sources.contains(&Source::Value(parameter.name.clone())) {
                    return Err(Problem::refused(
                        argument.map_or(call.place, |argument| argument.value.place),
                        format!(
                            "the result borrows from `${}`, but this value has no name",
                            parameter.name
                        ),
                        "a fresh value ends with its line, so a result can't borrow from it; give it a name first (D139)",
                    )
                    .because(function.place, format!("`{name}`'s result is set here")));
                }
                continue;
            };
            let held = Self::type_of(known);
            let sources = match &held {
                TypeSpec::Borrow { sources, .. } => sources.clone(),
                _ => vec![Source::Value(argument_name.clone())],
            };
            values.insert(parameter.name.clone(), sources);
            bind_sources(&parameter.type_spec, held.owned(), &mut named);
        }
        for condition in &function.requires {
            let ExpressionKind::LastsAtLeast { longer, shorter } = &condition.kind else {
                continue;
            };
            let (Some(long), Some(short)) = (values.get(longer), values.get(shorter)) else {
                return Err(Problem::not_decided(
                    call.place,
                    "a value with no name where a `requires` compares sources",
                ));
            };
            self.refuse_shorter(
                long,
                short,
                call.place,
                condition.place,
                "this `requires` line",
            )?;
        }
        for declaration in &function.sources {
            for (shorter, rule_place) in &declaration.lasts_at_least {
                let long = named.get(&declaration.name).cloned().unwrap_or_default();
                let short = named.get(shorter).cloned().unwrap_or_default();
                self.refuse_shorter(&long, &short, call.place, *rule_place, "this rule")?;
            }
        }
        Ok(Some(substitute(result, &values, &named)))
    }

    /// Refuses a call whose arguments don't meet a `lasts_at_least` fact (D127, D132).
    fn refuse_shorter(
        &self,
        longer: &[Source],
        shorter: &[Source],
        place: Place,
        fact: Place,
        what: &str,
    ) -> Result<(), Problem> {
        for long in longer {
            for short in shorter {
                if !self.lasts(long, short) {
                    return Err(Problem::refused(
                        place,
                        format!(
                            "{} may not last as long as {}",
                            long.describe(),
                            short.describe()
                        ),
                        "every call meets the `lasts_at_least` facts its function states (D127, D132)",
                    )
                    .because(fact, format!("{what} asks for it")));
                }
            }
        }
        Ok(())
    }
}

// Where values go: a borrow only where its type names its source.
impl Checker<'_> {
    /// Checks that a value's borrows fit where it goes (D125, D126, D130).
    pub(super) fn borrow_into(
        &self,
        value: &Expression,
        wanted: &TypeSpec,
        destination: &Destination<'_>,
    ) -> Result<(), Problem> {
        if let (
            ExpressionKind::Array { items, .. },
            TypeSpec::Array {
                element: wanted, ..
            },
        ) = (&value.kind, wanted)
        {
            for item in items {
                self.borrow_into(item, wanted, destination)?;
            }
            return Ok(());
        }
        let value_type = self.value_type(value)?;
        match fits(value_type.as_ref(), wanted) {
            None => Ok(()),
            Some(misfit) => Err(self.misfit(misfit, value, destination)),
        }
    }

    fn misfit(&self, misfit: Misfit, value: &Expression, destination: &Destination<'_>) -> Problem {
        let into = destination.describe();
        match misfit {
            Misfit::Extra(Source::Value(name), _)
                if matches!(destination, Destination::Result) && !self.is_parameter(&name) =>
            {
                let problem = Problem::refused(
                    value.place,
                    format!("this borrows from `${name}`, which ends when the function returns"),
                    "a result borrows from a parameter, a function's own source or `program` (D49, D127, D136)",
                );
                match self.lookup(&name) {
                    Some(known) => {
                        problem.because(known.place, format!("`${name}` is declared here"))
                    }
                    None => problem,
                }
            }
            Misfit::Extra(source, wanted) if wanted == [Source::Program] => Problem::refused(
                value.place,
                format!(
                    "this borrows from {}, but {into} must last until the program ends",
                    source.describe()
                ),
                "only `program` lasts until the program ends; a name ends with its block (D127)",
            ),
            Misfit::Extra(source, wanted) => Problem::refused(
                value.place,
                format!(
                    "this borrows from {}, but {into} borrows from {}",
                    source.describe(),
                    describe_sources(&wanted)
                ),
                "a borrow goes only where its type names its source (D130)",
            ),
            Misfit::BorrowIntoOwned => Problem::refused(
                value.place,
                format!("this is a borrow, but {into} holds a value of its own"),
                "a borrow is stored only where the type says `Lend from` (D125, D126)",
            ),
            Misfit::OwnedIntoBorrow => Problem::refused(
                value.place,
                format!("this is a value of its own, but {into} holds a borrow"),
                "a borrow is made with `Lend`, from a named value (D126)",
            ),
        }
    }

    /// The full type at the end of `$x.a[pos: 2]`, with the struct's sources filled in.
    pub(super) fn path_type(&self, root: &TypeSpec, steps: &[Step]) -> Option<TypeSpec> {
        let mut current = root.clone();
        for step in steps {
            current = match (step, current.owned()) {
                (Step::Item(_), TypeSpec::Array { element, .. }) => element.as_ref().clone(),
                (
                    Step::Field(name),
                    TypeSpec::Named {
                        name: type_name,
                        fills,
                    },
                ) => {
                    let definition = self.types.structure(type_name)?;
                    let field = definition.fields.iter().find(|field| &field.name == name)?;
                    let bound: HashMap<String, Vec<Source>> = fills
                        .iter()
                        .map(|fill| (fill.name.clone(), fill.sources.clone()))
                        .collect();
                    substitute(&field.type_spec, &HashMap::new(), &bound)
                }
                _ => return None,
            };
        }
        Some(current)
    }

    /// Whether `$x.a[pos: 2]` reaches through a borrow, which is read-only (D80, D135).
    pub(super) fn through_borrow(&self, root: &TypeSpec, steps: &[Step]) -> bool {
        let mut current = root.clone();
        for step in steps {
            if matches!(current, TypeSpec::Borrow { .. }) {
                return true;
            }
            let Some(next) = self.path_type(&current, std::slice::from_ref(step)) else {
                return false;
            };
            current = next;
        }
        false
    }
}

// While a borrow lasts (D5, D129).
impl Checker<'_> {
    /// Refuses changing, giving away or lending to change `$name` while a borrow of it is used later (D5, D129).
    /// `same_call` holds the call's other arguments, which use their values at the same time.
    pub(super) fn refuse_while_lent(
        &self,
        name: &str,
        place: Place,
        doing: &str,
        same_call: &[&Expression],
    ) -> Result<(), Problem> {
        for (scope_index, scope) in self.scopes.iter().enumerate() {
            for (holder, known) in scope {
                if holder == name {
                    continue;
                }
                let Some(type_spec) = &known.type_spec else {
                    continue;
                };
                let mut sources = Vec::new();
                collect_sources(type_spec, &mut sources);
                if !sources.contains(&Source::Value(name.to_owned())) {
                    continue;
                }
                let later = same_call
                    .iter()
                    .find_map(|expression| expression_mentions(expression, holder))
                    .map(|place| (place, false))
                    .or_else(|| self.used_later(holder, scope_index));
                let Some((used, replaced)) = later else {
                    continue;
                };
                if replaced {
                    return Err(Problem::not_built(
                        used,
                        "following a borrow's holder that is given a new borrow later",
                    ));
                }
                return Err(Problem::refused(
                    place,
                    format!("`${name}` {doing} while `${holder}` still borrows it"),
                    "while a borrow is still used, its source can't change, be given away or be lent to change (D5, D129)",
                )
                .because(known.place, format!("`${holder}` borrows `${name}` here"))
                .because(used, format!("`${holder}` is used again here")));
            }
        }
        Ok(())
    }

    /// Where `$holder` is next used after the statement being checked, on any path (D144): later in its blocks, or
    /// anywhere in a loop around them that it was declared outside of, since the next trip comes after. The flag says
    /// the use is a whole new value for it.
    fn used_later(&self, holder: &str, holder_scope: usize) -> Option<(Place, bool)> {
        for (rest, block_scope) in self.rest.iter().rev() {
            if *block_scope < holder_scope {
                continue;
            }
            if let Some(found) = statements_mention(rest, holder) {
                return Some(found);
            }
        }
        for (body, loop_scope) in self.loop_bodies.iter().rev() {
            if holder_scope < *loop_scope
                && let Some(found) = statements_mention(body, holder)
            {
                return Some(found);
            }
        }
        None
    }
}

/// A type with the shape of `kind` and no borrows. Integer bounds play no part in where borrows come from, so they are
/// left at zero.
fn shape_spec(kind: &ValueKind) -> TypeSpec {
    match kind {
        ValueKind::Integer => TypeSpec::Integer(Bounds {
            min: BigInt::ZERO,
            max: BigInt::ZERO,
        }),
        ValueKind::Bool => TypeSpec::Bool,
        ValueKind::Named(name) => TypeSpec::Named {
            name: name.clone(),
            fills: Vec::new(),
        },
        ValueKind::Array { element, length } => TypeSpec::Array {
            element: Box::new(shape_spec(element)),
            length: length.clone(),
        },
    }
}

/// Learns what a declared type's named sources stand for, by laying a value's full type over it.
fn bind_sources(declared: &TypeSpec, actual: &TypeSpec, bound: &mut HashMap<String, Vec<Source>>) {
    match (declared, actual) {
        (
            TypeSpec::Borrow { sources, inner, .. },
            TypeSpec::Borrow {
                sources: actual_sources,
                inner: actual_inner,
                ..
            },
        ) => {
            for source in sources {
                if let Source::Named(name) = source {
                    add_sources(bound.entry(name.clone()).or_default(), actual_sources);
                }
            }
            bind_sources(inner, actual_inner, bound);
        }
        (
            TypeSpec::Named { fills, .. },
            TypeSpec::Named {
                fills: actual_fills,
                ..
            },
        ) => {
            for fill in fills {
                let Some(actual) = actual_fills.iter().find(|other| other.name == fill.name) else {
                    continue;
                };
                for source in &fill.sources {
                    if let Source::Named(name) = source {
                        add_sources(bound.entry(name.clone()).or_default(), &actual.sources);
                    }
                }
            }
        }
        (
            TypeSpec::Array { element, .. },
            TypeSpec::Array {
                element: actual, ..
            },
        ) => bind_sources(element, actual, bound),
        _ => {}
    }
}

/// Replaces `$parameter` and `` `name` `` sources by what they stand for.
fn substitute(
    type_spec: &TypeSpec,
    values: &HashMap<String, Vec<Source>>,
    named: &HashMap<String, Vec<Source>>,
) -> TypeSpec {
    let replace = |sources: &[Source]| {
        let mut replaced = Vec::new();
        for source in sources {
            let found = match source {
                Source::Value(name) => values.get(name),
                Source::Named(name) => named.get(name),
                Source::Program => None,
            };
            match found {
                Some(found) => add_sources(&mut replaced, found),
                None => add_sources(&mut replaced, std::slice::from_ref(source)),
            }
        }
        replaced
    };
    match type_spec {
        TypeSpec::Borrow {
            exclusive,
            sources,
            inner,
        } => TypeSpec::Borrow {
            exclusive: *exclusive,
            sources: replace(sources),
            inner: Box::new(substitute(inner, values, named)),
        },
        TypeSpec::Named { name, fills } => TypeSpec::Named {
            name: name.clone(),
            fills: fills
                .iter()
                .map(|fill| SourceFill {
                    name: fill.name.clone(),
                    sources: replace(&fill.sources),
                })
                .collect(),
        },
        TypeSpec::Array { element, length } => TypeSpec::Array {
            element: Box::new(substitute(element, values, named)),
            length: length.clone(),
        },
        other => other.clone(),
    }
}

/// Whether a value with full type `value` (`None`: it holds no borrow) fits a holder of type `wanted`: every source it
/// borrows from is one `wanted` names (D130), and a borrow goes only where a borrow is held (D125, D126).
fn fits(value: Option<&TypeSpec>, wanted: &TypeSpec) -> Option<Misfit> {
    match (value, wanted) {
        (
            Some(TypeSpec::Borrow { sources, inner, .. }),
            TypeSpec::Borrow {
                sources: wanted_sources,
                inner: wanted_inner,
                ..
            },
        ) => sources
            .iter()
            .find(|source| !wanted_sources.contains(source))
            .map(|source| Misfit::Extra(source.clone(), wanted_sources.clone()))
            .or_else(|| fits(Some(inner), wanted_inner)),
        (Some(TypeSpec::Borrow { .. }), _) => Some(Misfit::BorrowIntoOwned),
        (_, TypeSpec::Borrow { .. }) => Some(Misfit::OwnedIntoBorrow),
        (
            Some(TypeSpec::Named { fills, .. }),
            TypeSpec::Named {
                fills: wanted_fills,
                ..
            },
        ) => fills.iter().find_map(|fill| {
            let wanted_sources = wanted_fills
                .iter()
                .find(|other| other.name == fill.name)
                .map(|other| other.sources.clone())
                .unwrap_or_default();
            fill.sources
                .iter()
                .find(|source| !wanted_sources.contains(source))
                .map(|source| Misfit::Extra(source.clone(), wanted_sources.clone()))
        }),
        (
            Some(TypeSpec::Array { element, .. }),
            TypeSpec::Array {
                element: wanted, ..
            },
        ) => fits(Some(element), wanted),
        _ => None,
    }
}

/// Adds sources not already there.
fn add_sources(into: &mut Vec<Source>, sources: &[Source]) {
    for source in sources {
        if !into.contains(source) {
            into.push(source.clone());
        }
    }
}

/// Joins two values' borrows, for the items of one array.
fn merge(into: &mut TypeSpec, other: &TypeSpec) {
    match (into, other) {
        (
            TypeSpec::Borrow { sources, inner, .. },
            TypeSpec::Borrow {
                sources: other_sources,
                inner: other_inner,
                ..
            },
        ) => {
            add_sources(sources, other_sources);
            merge(inner, other_inner);
        }
        (
            TypeSpec::Named { fills, .. },
            TypeSpec::Named {
                fills: other_fills, ..
            },
        ) => {
            for fill in fills.iter_mut() {
                if let Some(other) = other_fills.iter().find(|other| other.name == fill.name) {
                    add_sources(&mut fill.sources, &other.sources);
                }
            }
        }
        (TypeSpec::Array { element, .. }, TypeSpec::Array { element: other, .. }) => {
            merge(element, other);
        }
        _ => {}
    }
}

fn describe_sources(sources: &[Source]) -> String {
    let written: Vec<String> = sources.iter().map(Source::describe).collect();
    match written.as_slice() {
        [one] => one.clone(),
        _ => format!("<{}>", written.join(", ")),
    }
}

/// The first place in `statements` that uses `$name`, and whether that use gives it a whole new value.
fn statements_mention(statements: &[Statement], name: &str) -> Option<(Place, bool)> {
    statements
        .iter()
        .find_map(|statement| statement_mentions(statement, name))
}

fn statement_mentions(statement: &Statement, name: &str) -> Option<(Place, bool)> {
    let in_expressions = |expressions: &[&Expression]| {
        expressions
            .iter()
            .find_map(|expression| expression_mentions(expression, name))
            .map(|place| (place, false))
    };
    match &statement.kind {
        StatementKind::Declare { value, .. } => in_expressions(&[value]),
        StatementKind::Assign { target, value } => {
            let indexes: Vec<&Expression> = target
                .steps
                .iter()
                .filter_map(|step| match step {
                    Step::Item(index) => Some(index.value.as_ref()),
                    Step::Field(_) => None,
                })
                .collect();
            in_expressions(&[value])
                .or_else(|| in_expressions(&indexes))
                .or_else(|| {
                    (target.name == name).then_some((statement.place, target.steps.is_empty()))
                })
        }
        StatementKind::Call(call) => in_expressions(&call_values(call)),
        StatementKind::If {
            branches,
            else_body,
        } => branches
            .iter()
            .find_map(|branch| {
                in_expressions(&[&branch.condition])
                    .or_else(|| statements_mention(&branch.body, name))
            })
            .or_else(|| {
                else_body
                    .as_ref()
                    .and_then(|body| statements_mention(body, name))
            }),
        StatementKind::For { source, body, .. } => {
            let found = match source {
                LoopSource::Range(range) => {
                    let end = match &range.end {
                        RangeEnd::To(end) | RangeEnd::Before(end) => end,
                    };
                    let mut values = vec![&range.from, end];
                    values.extend(range.step_by.as_ref());
                    in_expressions(&values)
                }
                LoopSource::Each(collection) => in_expressions(&[collection]),
            };
            found.or_else(|| statements_mention(body, name))
        }
        StatementKind::While {
            condition,
            decreases,
            body,
        } => {
            let mut values = vec![condition];
            values.extend(decreases.as_ref());
            in_expressions(&values).or_else(|| statements_mention(body, name))
        }
        StatementKind::Forever { body, .. } => statements_mention(body, name),
        StatementKind::Break { .. } => None,
        StatementKind::Return { value } => {
            value.as_ref().and_then(|value| in_expressions(&[value]))
        }
        StatementKind::Match { subject, cases } => in_expressions(&[subject]).or_else(|| {
            cases
                .iter()
                .find_map(|case| statements_mention(&case.body, name))
        }),
    }
}

fn call_values(call: &Call) -> Vec<&Expression> {
    call.arguments
        .iter()
        .map(|argument| &argument.value)
        .collect()
}

/// The first place in `expression` that uses `$name`.
pub(super) fn expression_mentions(expression: &Expression, name: &str) -> Option<Place> {
    let place = expression.place;
    match &expression.kind {
        ExpressionKind::Value(found) | ExpressionKind::Old(found) => {
            (found == name).then_some(place)
        }
        ExpressionKind::Handoff { name: found, .. } => (found == name).then_some(place),
        ExpressionKind::LastsAtLeast { longer, shorter } => {
            (longer == name || shorter == name).then_some(place)
        }
        ExpressionKind::Call(call) => call_values(call)
            .into_iter()
            .find_map(|value| expression_mentions(value, name)),
        ExpressionKind::Build { fields, .. } => fields
            .iter()
            .find_map(|field| expression_mentions(&field.value, name)),
        ExpressionKind::Field { base, .. } => expression_mentions(base, name),
        ExpressionKind::Array { items, .. } => items
            .iter()
            .find_map(|item| expression_mentions(item, name)),
        ExpressionKind::Item { base, index } => {
            expression_mentions(base, name).or_else(|| expression_mentions(&index.value, name))
        }
        ExpressionKind::Negate(inner) | ExpressionKind::Not(inner) => {
            expression_mentions(inner, name)
        }
        ExpressionKind::Binary { left, right, .. } => {
            expression_mentions(left, name).or_else(|| expression_mentions(right, name))
        }
        ExpressionKind::Integer(_)
        | ExpressionKind::Bool(_)
        | ExpressionKind::Text(_)
        | ExpressionKind::Newline
        | ExpressionKind::Result => None,
    }
}
// end section oracle.checker.borrows
