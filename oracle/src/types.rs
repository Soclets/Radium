// section oracle.types:
//! The program's own types, `struct` (D58) and `union.tagged` (D59): checks their definitions and answers questions
//! about them for the checker and the interpreter.

use std::collections::HashMap;

use crate::ast::{Case, Field, Program, StructDefinition, TypeSpec, UnionDefinition, ValueKind};
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
            types.check_fields(&definition.fields)?;
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

    /// Checks that a written type exists and has sensible bounds (D14).
    pub(crate) fn check_type(&self, type_spec: &TypeSpec, place: Place) -> Result<(), Problem> {
        match type_spec {
            TypeSpec::Integer(bounds) if bounds.min > bounds.max => Err(Problem::refused(
                place,
                format!("`min: {}` is above `max: {}`", bounds.min, bounds.max),
                "an integer's bounds go from `min` up to `max` (D14)",
            )),
            TypeSpec::Integer(_) | TypeSpec::Bool => Ok(()),
            TypeSpec::Named(name) => {
                if self.structs.contains_key(name.as_str())
                    || self.unions.contains_key(name.as_str())
                {
                    Ok(())
                } else {
                    Err(Problem::refused(
                        place,
                        format!("there is no type named `{name}`"),
                        "a type is `int[…]`, `bool`, `array[…]` or a declared `struct` or `union.tagged` (D58, D59, D70)",
                    ))
                }
            }
            TypeSpec::Array { element, .. } => self.check_type(element, place),
        }
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
            TypeSpec::Named(inner) => inner == wanted || self.contains(wanted, inner, seen),
            TypeSpec::Array { element, .. } => self.type_contains(wanted, element, seen),
            TypeSpec::Integer(_) | TypeSpec::Bool => false,
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
// end section oracle.types
