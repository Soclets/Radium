// section oracle.binding:
//! Matches a call's arguments to the function's parameters, by position and by label (D54).

use crate::ast::{Call, Function, Parameter};
use crate::outcome::Problem;

/// Where a parameter's value comes from in one call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Binding {
    /// The call's argument at this position.
    Argument(usize),
    /// The function's `defaults` entry at this position.
    Default(usize),
}

/// Gives every parameter of `function` its value's source in `call`, one per parameter, in parameter order.
///
/// Unlabelled arguments fill the unlabelled parameters in order; a labelled parameter must be passed by its label
/// (labels are required once declared, D54) unless it has a default. With bare `labels`, arguments keep the declared
/// order; `labels.any_order` frees the labelled ones.
pub(crate) fn bind_arguments(function: &Function, call: &Call) -> Result<Vec<Binding>, Problem> {
    let label_of = |parameter: &Parameter| {
        function
            .labels
            .as_ref()
            .and_then(|labels| {
                labels
                    .entries
                    .iter()
                    .find(|entry| entry.parameter == parameter.name)
            })
            .map(|entry| entry.label.as_str())
    };
    let any_order = function
        .labels
        .as_ref()
        .is_some_and(|labels| labels.any_order);
    let mut bindings: Vec<Option<Binding>> = vec![None; function.parameters.len()];
    let mut next_unlabelled = 0_usize;
    let mut last_in_order: Option<usize> = None;
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        let parameter_index = if let Some(label) = &argument.label {
            function
                .parameters
                .iter()
                .position(|parameter| label_of(parameter) == Some(label.as_str()))
                .ok_or_else(|| {
                    Problem::refused(
                        argument.line,
                        format!("`{}` has no label `{label}:` (D54)", function.name),
                    )
                })?
        } else {
            let found = function
                .parameters
                .iter()
                .enumerate()
                .skip(next_unlabelled)
                .find(|(_, parameter)| label_of(parameter).is_none())
                .map(|(index, _)| index);
            let Some(index) = found else {
                let labelled = function
                    .parameters
                    .iter()
                    .skip(next_unlabelled)
                    .find_map(&label_of);
                let message = labelled.map_or_else(
                    || format!("this call gives `{}` more arguments than it takes", function.name),
                    |label| format!("this argument needs its label, `{label}:`; labels are required once declared (D54)"),
                );
                return Err(Problem::refused(argument.line, message));
            };
            next_unlabelled = index.saturating_add(1);
            index
        };
        if !any_order || argument.label.is_none() {
            if last_in_order.is_some_and(|last| parameter_index <= last) {
                return Err(Problem::refused(
                    argument.line,
                    "arguments go in the declared order unless the function says `labels.any_order` (D54)",
                ));
            }
            last_in_order = Some(parameter_index);
        }
        let Some(slot) = bindings.get_mut(parameter_index) else {
            return Err(Problem::refused(
                argument.line,
                "this argument matches no parameter",
            ));
        };
        if slot.is_some() {
            return Err(Problem::refused(
                argument.line,
                "this parameter is given twice",
            ));
        }
        *slot = Some(Binding::Argument(argument_index));
    }
    function
        .parameters
        .iter()
        .zip(bindings)
        .map(|(parameter, binding)| match binding {
            Some(binding) => Ok(binding),
            None => missing(function, parameter, label_of(parameter), call.line),
        })
        .collect()
}

fn missing(
    function: &Function,
    parameter: &Parameter,
    label: Option<&str>,
    line: usize,
) -> Result<Binding, Problem> {
    if let Some(index) = function
        .defaults
        .iter()
        .position(|default| default.parameter == parameter.name)
    {
        return Ok(Binding::Default(index));
    }
    let wanted = label.map_or_else(
        || format!("`${}`", parameter.name),
        |label| format!("`{label}:`"),
    );
    Err(Problem::refused(
        line,
        format!("this call to `{}` is missing {wanted} (D54)", function.name),
    ))
}
// end section oracle.binding
