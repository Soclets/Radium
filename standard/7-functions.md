# 7. Functions

## 7.1 The header

1. A function is `fn name [parameters] -> result:` … `end fn name` (D13, D50, D52).
2. **Every function states its result after `->`** (D98). One that gives nothing back writes `-> void` (D99). A
   function with no `->` is refused.
3. Parameters go inside `[ ]`, since a definition describes (D50); each is `$name: type`, separated by `,`. A
   function with none writes `[]`.
4. A parameter can't be changed inside its function: it is `var.immut` by default (D31). Writing `var.immut` is refused
   as a spelled-out default (D39). `var $name` may change; `var.mut $name` must change (D30, D31).
5. Parameter and result types write their bounds; `infer` isn't allowed there (D14, D108).
   *Tests:* `missing_arrow_refused`, `parameter_is_immut`, `infer_only_for_locals`, `function_and_contracts`.

## 7.2 Kinds of function

1. `fn` promises nothing either way about side effects (D51).
2. `fn.pure` is a function in the maths sense: no side effects, the same result for the same arguments (D51). It can't
   print, and can call only other `fn.pure` functions.
3. `fn.main` is where the program starts (D101); see 1.1.
   *Tests:* `pure_cannot_print`.

## 7.3 The top of the body

1. The first lines of the body are, in this order and each line at most once: `labels`, then `defaults`, then the
   contracts. A contract after any other line is refused (D53, D54).
2. **`labels [$limit as limit];`**: the caller writes `limit:` before that argument (D54). Once declared, the label
   is required. Bare `labels` keeps the declared order; `labels.any_order` lets labelled arguments come in any order.
   Writing `labels.in_order` is refused (D39).
3. **`defaults [$limit = 40];`**: only labelled parameters may have defaults, and defaults are the only optional
   arguments (D54). A default other than a plain number, `true` or `false`: **not decided**.
4. **Contracts** (D53): `requires …;` is what callers must guarantee; `ensures …;` is what the function promises back.
   In an `ensures`, `result` is the value given back, and `old[$x]` is a parameter's value when the function started.
   Both are refused anywhere else.
5. **Contracts may call only `fn.pure` functions**, so a contract never does anything (D51).
6. In mode 2 a `requires` is checked at each call and an `ensures` at each return; in the final build both are
   proven (D9, D51, D53).
   *Tests:* `labels_and_defaults`, `label_required`, `labels_keep_their_order`, `labels_any_order`,
   `default_needs_label`, `contracts_come_first`, `requires_not_met`, `ensures_and_old`, `ensures_broken`,
   `ensures_result_broken`, `result_only_in_ensures`, `contracts_call_pure`, `contracts_call_pure_only`.

## 7.4 Giving back a result

1. **`return value;`** ends the function and gives back the value; it is checked against the result type's bounds
   (D14, D95).
2. A `void` function returns with **`return void;`**. A bare `return;` is refused (D100). `return void;` in a function
   with a result, and `return value;` in a `void` function, are refused.
3. **Every path through every function ends in `return`** (D96, D102), judged by the code's shape: the last line
   returns; or it is an `if` … `else` whose every branch returns; or a `forever` that nothing breaks out of. A loop
   that might run zero times, or an `if` without `else`, doesn't count. Reaching `end fn` is never an implied return.
   *Tests:* `return_gives_result`, `result_outside_bounds`, `return_from_inside_loops`, `every_branch_returns`,
   `forever_returns`, `return_on_every_path`, `early_return_void`, `bare_return_refused`,
   `return_void_needs_void_function`, `return_without_result`, `return_value_from_main_refused`,
   `void_needs_return_void`.

## 7.5 Calling

1. A call runs something, so it uses `( )` (D50): `first_word($text, limit: 20)`.
2. **A result can't be ignored**: calling a function with a result as a statement on its own is refused (D97). A way
   to throw a result away on purpose: **not decided**.
3. A `void` function can't be used as a value (D99).
4. A writer's function with the same name as a Radium word is called with `my.`, as in `my.x(…)` (D43). Writing `my.`
   when nothing clashes: **not decided**.
5. A function may call itself. It must provably end (D7); how a recursive function shows it (`decreases`, D4) isn't in
   the oracle yet.
   *Tests:* `result_cannot_be_ignored`, `nothing_to_use_as_value`, `my_mark_on_clash`, `clash_needs_my`,
   `recursion`.

## 7.6 `std::print`

1. `std::print(…)` writes its items to standard output (D26, D27). `newline` writes a line break (D94).
2. How items are joined (the oracle prints them one after another with nothing between) and whether `print` adds a
   line break of its own: **not decided**.
   *Tests:* `hello`, `newln_written_in_full`.
