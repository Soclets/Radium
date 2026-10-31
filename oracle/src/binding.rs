// section oracle.binding:
//! Matches a call's arguments to the function's parameters, by position and by label (D54).

use crate::ast::{Argument, Call, Function, Parameter};
use crate::outcome::{Place, Problem};

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
    let any_order = function
        .labels
        .as_ref()
        .is_some_and(|labels| labels.any_order);
    let mut bindings: Vec<Option<Binding>> = vec![None; function.parameters.len()];
    let mut next_unlabelled = 0_usize;
    let mut last_in_order: Option<usize> = None;
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        let parameter_index = if let Some(label) = &argument.label {
            labelled_index(function, label, argument)?
        } else {
            let index = unlabelled_index(function, next_unlabelled, argument)?;
            next_unlabelled = index.saturating_add(1);
            index
        };
        if !any_order || argument.label.is_none() {
            if last_in_order.is_some_and(|last| parameter_index <= last) {
                return Err(Problem::refused(
                    argument.place,
                    "this argument is out of order",
                    "arguments go in the declared order unless the function says `labels.any_order` (D54)",
                )
                .because(
                    function.place,
                    format!("`{}` declares its parameters here", function.name),
                ));
            }
            last_in_order = Some(parameter_index);
        }
        let Some(slot) = bindings.get_mut(parameter_index) else {
            return Err(Problem::refused(
                argument.place,
                "this argument matches no parameter",
                "every argument matches one parameter (D54)",
            ));
        };
        if slot.is_some() {
            return Err(Problem::refused(
                argument.place,
                "this parameter is given twice",
                "every argument matches one parameter, once (D54)",
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
            None => missing(
                function,
                parameter,
                label_of(function, parameter),
                call.place,
            ),
        })
        .collect()
}

/// The label callers write for `parameter`, if it has one.
fn label_of<'function>(
    function: &'function Function,
    parameter: &Parameter,
) -> Option<&'function str> {
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
}

/// The parameter a labelled argument names.
fn labelled_index(function: &Function, label: &str, argument: &Argument) -> Result<usize, Problem> {
    function
        .parameters
        .iter()
        .position(|parameter| label_of(function, parameter) == Some(label))
        .ok_or_else(|| {
            let problem = Problem::refused(
                argument.place,
                format!("`{}` has no label `{label}:`", function.name),
                "a call uses the labels its function declares (D54)",
            );
            match &function.labels {
                Some(labels) => problem.because(
                    labels.place,
                    format!("`{}`'s labels are declared here", function.name),
                ),
                None => problem.because(
                    function.place,
                    format!("`{}` declares no labels", function.name),
                ),
            }
        })
}

/// The next unlabelled parameter, from `from` on, for an argument with no label.
fn unlabelled_index(
    function: &Function,
    from: usize,
    argument: &Argument,
) -> Result<usize, Problem> {
    let found = function
        .parameters
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, parameter)| label_of(function, parameter).is_none())
        .map(|(index, _)| index);
    if let Some(index) = found {
        return Ok(index);
    }
    let labelled = function
        .parameters
        .iter()
        .skip(from)
        .find_map(|parameter| label_of(function, parameter));
    let problem = labelled.map_or_else(
        || {
            Problem::refused(
                argument.place,
                format!("`{}` takes fewer arguments than this", function.name),
                "every argument matches one parameter (D54)",
            )
        },
        |label| {
            Problem::refused(
                argument.place,
                "this argument has no label",
                "a labelled parameter is always passed with its label (D54)",
            )
            .fix(format!("`{label}: …`"))
        },
    );
    Err(problem.because(
        function.place,
        format!("`{}` is declared here", function.name),
    ))
}

fn missing(
    function: &Function,
    parameter: &Parameter,
    label: Option<&str>,
    place: Place,
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
        place,
        format!("this call to `{}` is missing {wanted}", function.name),
        "every parameter without a default gets an argument (D54)",
    )
    .because(
        parameter.place,
        format!("`${}` is declared here", parameter.name),
    ))
}
// end section oracle.binding
