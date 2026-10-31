// section oracle.types:
//! The program's own types, `struct` (D58) and `union.tagged` (D59): checks their definitions, including the named
//! sources a struct's borrows come from (D126, D127, D133, D134), and answers questions about them for the checker and
//! the interpreter.

use std::collections::HashMap;

use crate::ast::{
    Case, Field, Program, Source, SourceDeclaration, StructDefinition, TypeSpec, UnionDefinition,
    ValueKind,
};
use crate::outcome::{Place, Problem};
use crate::rules::is_radium_word;

/// Every type the program declares, by name.
pub(crate) struct Types<'program> {
    structs: HashMap<&'program str, &'program StructDefinition>,
    unions: HashMap<&'program str, &'program UnionDefinition>,
}

impl<'program> Types<'program> {
    /// Collects and checks the program's type definitions.
    pub(crate) fn new(program: &'program Program) -> Result<Self, Problem> {
        let mut types = Self {
            structs: HashMap::new(),
            unions: HashMap::new(),
        };
        let mut places: HashMap<&str, Place> = HashMap::new();
        let named = program
            .structs
            .iter()
            .map(|definition| (definition.name.as_str(), definition.place))
            .chain(
                program
                    .unions
                    .iter()
                    .map(|definition| (definition.name.as_str(), definition.place)),
            );
        for (name, place) in named {
            if is_radium_word(name) {
                return Err(Problem::not_decided(
                    place,
                    format!("a type named `{name}`, which is also a Radium word (D43)"),
                ));
            }
            if let Some(first) = places.insert(name, place) {
                return Err(
                    Problem::not_decided(place, format!("two types named `{name}`"))
                        .because(first, format!("the first `{name}` is here")),
                );
            }
        }
        for definition in &program.structs {
            types.structs.insert(definition.name.as_str(), definition);
        }
        for definition in &program.unions {
            types.unions.insert(definition.name.as_str(), definition);
        }
        for definition in &program.structs {
            check_source_declarations(&definition.sources)?;
            types.check_fields(&definition.fields)?;
            types.check_struct_sources(definition)?;
            types.refuse_self_containing(&definition.name, definition.place)?;
        }
        for definition in &program.unions {
            let mut seen: HashMap<&str, Place> = HashMap::new();
            for case in &definition.cases {
                if let Some(first) = seen.insert(case.name.as_str(), case.place) {
                    return Err(Problem::refused(
                        case.place,
                        format!("a second case named `{}`", case.name),
                        "each case of a `union.tagged` has its own name (D59)",
                    )
                    .because(first, "the first one is here"));
                }
                types.check_fields(&case.fields)?;
                if let Some(field) = case
                    .fields
                    .iter()
                    .find(|field| holds_a_borrow(&field.type_spec))
                {
                    return Err(Problem::not_decided(
                        field.place,
                        "a `union.tagged` case holding a borrow",
                    ));
                }
            }
            types.refuse_self_containing(&definition.name, definition.place)?;
        }
        Ok(types)
    }

    fn check_fields(&self, fields: &[Field]) -> Result<(), Problem> {
        let mut seen: HashMap<&str, Place> = HashMap::new();
        for field in fields {
            if let Some(first) = seen.insert(field.name.as_str(), field.place) {
                return Err(Problem::refused(
                    field.place,
                    format!("a second field named `{}`", field.name),
                    "each field has its own name (D58)",
                )
                .because(first, "the first one is here"));
            }
            self.check_type(&field.type_spec, field.place)?;
            if self.is_must_give(&field.type_spec.kind()) {
                return Err(Problem::not_decided(
                    field.place,
                    "a `must_Give` value held inside a field (D84)",
                ));
            }
        }
        Ok(())
    }

    /// Checks that a written type exists, has sensible bounds (D14), and fills in exactly the sources its struct
    /// declares, in their order (D126, D136). Which names the sources may be is the writer's context: see
    /// `check_struct_sources` and the checker.
    pub(crate) fn check_type(&self, type_spec: &TypeSpec, place: Place) -> Result<(), Problem> {
        match type_spec {
            TypeSpec::Integer(bounds) if bounds.min > bounds.max => Err(Problem::refused(
                place,
                format!("`min: {}` is above `max: {}`", bounds.min, bounds.max),
                "an integer's bounds go from `min` up to `max` (D14)",
            )),
            TypeSpec::Integer(_) | TypeSpec::Bool => Ok(()),
            TypeSpec::Named { name, fills } => {
                if let Some(definition) = self.structs.get(name.as_str()) {
                    check_fills(definition, fills, place)
                } else if self.unions.contains_key(name.as_str()) {
                    if fills.is_empty() {
                        Ok(())
                    } else {
                        Err(Problem::refused(
                            place,
                            format!("`{name}` has no sources to fill in"),
                            "a type fills in only the sources its definition declares (D126)",
                        ))
                    }
                } else {
                    Err(Problem::refused(
                        place,
                        format!("there is no type named `{name}`"),
                        "a type is `int[…]`, `bool`, `array<…>[…]`, a stored borrow or a declared `struct` or `union.tagged` (D58, D59, D70, D126)",
                    ))
                }
            }
            TypeSpec::Array { element, .. } => self.check_type(element, place),
            TypeSpec::Borrow {
                exclusive, inner, ..
            } => {
                self.check_type(inner, place)?;
                if inner.kind().is_copied() {
                    return Err(Problem::refused(
                        place,
                        "a borrow of a value that is always copied",
                        "integers and `bool` are always copied, so they are never lent (D92)",
                    ));
                }
                if *exclusive {
                    return Err(Problem::not_built(place, "storing a `Lend.mut` (D135)"));
                }
                Ok(())
            }
        }
    }

    /// A struct's fields borrow only from its own named sources or `program` (D126, D127); every source is used
    /// (D133); a field's struct type keeps that struct's `lasts_at_least` rules (D127).
    fn check_struct_sources(&self, definition: &StructDefinition) -> Result<(), Problem> {
        let declared = &definition.sources;
        let rules = source_rules(declared);
        let mut used: Vec<String> = Vec::new();
        for field in &definition.fields {
            let mut named = Vec::new();
            collect_sources(&field.type_spec, &mut named);
            for source in &named {
                match source {
                    Source::Value(name) => {
                        return Err(Problem::refused(
                            field.place,
                            format!("a field borrows from `${name}`"),
                            "a struct's field borrows from one of the struct's named sources, or `program` (D126, D127)",
                        ));
                    }
                    Source::Named(name) => {
                        if !declared.iter().any(|source| &source.name == name) {
                            return Err(Problem::refused(
                                field.place,
                                format!("`{}` declares no source `` `{name}` ``", definition.name),
                                "a field borrows only from sources its struct declares (D126)",
                            )
                            .because(
                                definition.place,
                                format!("`{}` is declared here", definition.name),
                            ));
                        }
                        used.push(name.clone());
                    }
                    Source::Program => {}
                }
            }
            self.check_nested_rules(
                &field.type_spec,
                field.place,
                &|longer, shorter| lasts_by_rules(&rules, longer, shorter),
                &|_, _| None,
            )?;
        }
        if let Some(unused) = declared.iter().find(|source| !used.contains(&source.name)) {
            return Err(Problem::refused(
                unused.place,
                format!("no field borrows from `` `{}` ``", unused.name),
                "every source a struct declares is used by a field (D133)",
            ));
        }
        Ok(())
    }

    /// Checks that a written struct type meets that struct's `lasts_at_least` rules, given how its sources were
    /// filled in and a way to compare two sources (D127). Goes inside borrows and arrays. `fix` gives the one fix
    /// for a pair, where there is one.
    pub(crate) fn check_nested_rules(
        &self,
        type_spec: &TypeSpec,
        place: Place,
        lasts: &dyn Fn(&Source, &Source) -> bool,
        fix: &dyn Fn(&Source, &Source) -> Option<String>,
    ) -> Result<(), Problem> {
        match type_spec {
            TypeSpec::Integer(_) | TypeSpec::Bool => Ok(()),
            TypeSpec::Array { element, .. } => self.check_nested_rules(element, place, lasts, fix),
            TypeSpec::Borrow { inner, .. } => self.check_nested_rules(inner, place, lasts, fix),
            TypeSpec::Named { name, fills } => {
                let Some(definition) = self.structs.get(name.as_str()) else {
                    return Ok(());
                };
                for declaration in &definition.sources {
                    for (shorter_name, rule_place) in &declaration.lasts_at_least {
                        let longer = fill_of(fills, &declaration.name);
                        let shorter = fill_of(fills, shorter_name);
                        for long in longer {
                            for short in shorter {
                                if !lasts(long, short) {
                                    let problem = match fix(long, short) {
                                        Some(fix) => Problem::refused(
                                            place,
                                            format!(
                                                "{} may not last as long as {}",
                                                long.describe(),
                                                short.describe()
                                            ),
                                            "a struct's sources keep its `lasts_at_least` rules, and a function states the ones its parameters must meet with `requires` (D127, D132)",
                                        )
                                        .fix(fix),
                                        None => Problem::refused(
                                            place,
                                            format!(
                                                "{} may not last as long as {}",
                                                long.describe(),
                                                short.describe()
                                            ),
                                            "a struct's sources keep its `lasts_at_least` rules (D127)",
                                        ),
                                    };
                                    return Err(problem.because(
                                        *rule_place,
                                        format!(
                                            "`{name}` needs `` `{}` `` to last at least as long as `` `{shorter_name}` ``",
                                            declaration.name
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }

    /// The sources a struct declares, in order.
    pub(crate) fn sources(&self, name: &str) -> &'program [SourceDeclaration] {
        self.structs
            .get(name)
            .map_or(&[], |definition| definition.sources.as_slice())
    }

    /// A struct or union is stored as one value, inside its holder, so it can't contain itself (D58, D70).
    fn refuse_self_containing(&self, name: &str, place: Place) -> Result<(), Problem> {
        let mut seen = Vec::new();
        if self.contains(name, name, &mut seen) {
            return Err(Problem::refused(
                place,
                format!("`{name}` contains itself"),
                "a value is stored whole, inside its holder, so a type can't hold itself (D58, D70)",
            ));
        }
        Ok(())
    }

    fn contains(&self, wanted: &str, name: &str, seen: &mut Vec<String>) -> bool {
        if seen.iter().any(|done| done == name) {
            return false;
        }
        seen.push(name.to_owned());
        let fields: Vec<&Field> = if let Some(definition) = self.structs.get(name) {
            definition.fields.iter().collect()
        } else if let Some(definition) = self.unions.get(name) {
            definition
                .cases
                .iter()
                .flat_map(|case| case.fields.iter())
                .collect()
        } else {
            Vec::new()
        };
        fields
            .iter()
            .any(|field| self.type_contains(wanted, &field.type_spec, seen))
    }

    fn type_contains(&self, wanted: &str, type_spec: &TypeSpec, seen: &mut Vec<String>) -> bool {
        match type_spec {
            TypeSpec::Named { name: inner, .. } => {
                inner == wanted || self.contains(wanted, inner, seen)
            }
            TypeSpec::Array { element, .. } => self.type_contains(wanted, element, seen),
            // A borrow points at its value; it doesn't store it inline.
            TypeSpec::Integer(_) | TypeSpec::Bool | TypeSpec::Borrow { .. } => false,
        }
    }

    /// The struct with this name.
    pub(crate) fn structure(&self, name: &str) -> Option<&'program StructDefinition> {
        self.structs.get(name).copied()
    }

    /// The union with this name.
    pub(crate) fn union(&self, name: &str) -> Option<&'program UnionDefinition> {
        self.unions.get(name).copied()
    }

    /// The case of a union, by name.
    pub(crate) fn case(&self, union_name: &str, case_name: &str) -> Option<&'program Case> {
        self.union(union_name)
            .and_then(|definition| definition.cases.iter().find(|case| case.name == case_name))
    }

    /// Whether a value of this kind must be handed off with `Give` exactly once (D84).
    pub(crate) fn is_must_give(&self, kind: &ValueKind) -> bool {
        let ValueKind::Named(name) = kind else {
            return false;
        };
        self.structure(name)
            .is_some_and(|definition| definition.must_give)
            || self
                .union(name)
                .is_some_and(|definition| definition.must_give)
    }
}

/// Each source has its own name (D134); a rule names a declared source (D127) and isn't written twice (D127).
pub(crate) fn check_source_declarations(declared: &[SourceDeclaration]) -> Result<(), Problem> {
    let mut seen: HashMap<&str, Place> = HashMap::new();
    for source in declared {
        if let Some(first) = seen.insert(source.name.as_str(), source.place) {
            return Err(Problem::refused(
                source.place,
                format!("a second source named `` `{}` ``", source.name),
                "each source has its own name (D134)",
            )
            .because(first, "the first one is here"));
        }
    }
    let mut facts: HashMap<(&str, &str), Place> = HashMap::new();
    for source in declared {
        for (shorter, place) in &source.lasts_at_least {
            if !declared.iter().any(|other| &other.name == shorter) {
                return Err(Problem::refused(
                    *place,
                    format!("there is no source `` `{shorter}` `` here"),
                    "`lasts_at_least` compares two sources declared in the same group (D127)",
                ));
            }
            if shorter == &source.name {
                return Err(Problem::not_decided(
                    *place,
                    "a source said to last at least as long as itself",
                ));
            }
            if let Some(first) = facts.insert((source.name.as_str(), shorter.as_str()), *place) {
                return Err(Problem::refused(
                    *place,
                    "the same `lasts_at_least` fact written twice",
                    "a fact is written once (D124, D127)",
                )
                .because(first, "it is written here first"));
            }
        }
    }
    Ok(())
}

/// The rules of a group as (longer, shorter) pairs.
pub(crate) fn source_rules(declared: &[SourceDeclaration]) -> Vec<(String, String)> {
    declared
        .iter()
        .flat_map(|source| {
            source
                .lasts_at_least
                .iter()
                .map(|(shorter, _)| (source.name.clone(), shorter.clone()))
        })
        .collect()
}

/// Whether `longer` lasts at least as long as `shorter`, where both are named sources of one group with `rules`
/// (D127). `program` lasts longest; a source lasts as long as itself; rules chain.
pub(crate) fn lasts_by_rules(
    rules: &[(String, String)],
    longer: &Source,
    shorter: &Source,
) -> bool {
    match (longer, shorter) {
        (Source::Program, _) => true,
        (_, Source::Program) => false,
        (Source::Named(longer), Source::Named(shorter)) => {
            let mut reached = vec![longer.clone()];
            let mut index = 0;
            while let Some(current) = reached.get(index).cloned() {
                if &current == shorter {
                    return true;
                }
                for (from, to) in rules {
                    if from == &current && !reached.contains(to) {
                        reached.push(to.clone());
                    }
                }
                index = index.saturating_add(1);
            }
            false
        }
        (longer, shorter) => longer == shorter,
    }
}

/// Fills in `fills` match the struct's sources one for one, in order (D126, D136).
fn check_fills(
    definition: &StructDefinition,
    fills: &[crate::ast::SourceFill],
    place: Place,
) -> Result<(), Problem> {
    for fill in fills {
        if !definition
            .sources
            .iter()
            .any(|source| source.name == fill.name)
        {
            return Err(Problem::refused(
                place,
                format!("`{}` has no source `` `{}` ``", definition.name, fill.name),
                "a type fills in only the sources its struct declares (D126)",
            )
            .because(
                definition.place,
                format!("`{}` is declared here", definition.name),
            ));
        }
    }
    let written: Vec<&str> = fills.iter().map(|fill| fill.name.as_str()).collect();
    let declared: Vec<&str> = definition
        .sources
        .iter()
        .map(|source| source.name.as_str())
        .collect();
    if let Some(missing) = declared.iter().find(|name| !written.contains(name)) {
        return Err(Problem::refused(
            place,
            format!("`{}`'s source `` `{missing}` `` isn't filled in", definition.name),
            "a borrow's sources are always written, never left for the checker to fill in (D5, D126)",
        )
        .because(definition.place, format!("`{}` is declared here", definition.name)));
    }
    if written != declared {
        return Err(Problem::refused(
            place,
            format!("`{}`'s sources are filled in out of order", definition.name),
            "every use keeps the order a group was declared in (D54, D136)",
        )
        .because(
            definition.place,
            format!("`{}` is declared here", definition.name),
        ));
    }
    Ok(())
}

/// The sources a struct type fills `name` with.
fn fill_of<'fills>(fills: &'fills [crate::ast::SourceFill], name: &str) -> &'fills [Source] {
    fills
        .iter()
        .find(|fill| fill.name == name)
        .map_or(&[], |fill| fill.sources.as_slice())
}

/// Every source named anywhere in a type.
pub(crate) fn collect_sources(type_spec: &TypeSpec, into: &mut Vec<Source>) {
    match type_spec {
        TypeSpec::Integer(_) | TypeSpec::Bool => {}
        TypeSpec::Array { element, .. } => collect_sources(element, into),
        TypeSpec::Borrow { sources, inner, .. } => {
            into.extend(sources.iter().cloned());
            collect_sources(inner, into);
        }
        TypeSpec::Named { fills, .. } => {
            for fill in fills {
                into.extend(fill.sources.iter().cloned());
            }
        }
    }
}

/// Whether a type holds a borrow anywhere.
pub(crate) fn holds_a_borrow(type_spec: &TypeSpec) -> bool {
    let mut sources = Vec::new();
    collect_sources(type_spec, &mut sources);
    !sources.is_empty()
}
// end section oracle.types
