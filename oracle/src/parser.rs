// section oracle.parser:
//! Builds the program's tree from tokens: blocks and `end` (D13), statements and `;` (D29), functions (D50–D54),
//! control flow (D71–D77) and expressions (D34–D40).

use crate::ast::{
    Argument, Bounds, Branch, Call, CallTarget, Changeability, Expression, ExpressionKind,
    Function, Holder, LabelEntry, Labels, LoopRange, Operator, Parameter, ParameterDefault,
    Program, RangeEnd, RemainderRule, Rounding, Statement, StatementKind, TypeSpec,
};
use num_bigint::{BigInt, Sign};

use crate::lexer::{Token, TokenKind, describe};
use crate::outcome::Problem;

/// Builds a whole program from its tokens.
pub(crate) fn parse(tokens: Vec<Token>) -> Result<Program, Problem> {
    let mut parser = Parser {
        tokens,
        position: 0,
    };
    parser.program()
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

/// What the top of a function's body holds: labels, defaults, then contracts (D53, D54).
struct BodyTop {
    labels: Option<Labels>,
    defaults: Vec<ParameterDefault>,
    requires: Vec<Expression>,
    ensures: Vec<Expression>,
}

impl Parser {
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn kind(&self) -> Option<&TokenKind> {
        self.current().map(|token| &token.kind)
    }

    fn kind_after(&self, offset: usize) -> Option<&TokenKind> {
        self.tokens
            .get(self.position.saturating_add(offset))
            .map(|token| &token.kind)
    }

    fn line(&self) -> usize {
        self.current()
            .or_else(|| self.tokens.last())
            .map_or(1, |token| token.line)
    }

    fn advance(&mut self) {
        self.position = self.position.saturating_add(1);
    }

    fn at(&self, kind: &TokenKind) -> bool {
        self.kind() == Some(kind)
    }

    fn at_word(&self, word: &str) -> bool {
        matches!(self.kind(), Some(TokenKind::Word(found)) if found == word)
    }

    fn at_any_word(&self) -> bool {
        matches!(self.kind(), Some(TokenKind::Word(_)))
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        let found = self.at(kind);
        if found {
            self.advance();
        }
        found
    }

    fn eat_word(&mut self, word: &str) -> bool {
        let found = self.at_word(word);
        if found {
            self.advance();
        }
        found
    }

    fn found(&self) -> String {
        self.kind()
            .map_or_else(|| "the end of the program".to_owned(), describe)
    }

    fn refuse(&self, message: impl Into<String>) -> Problem {
        Problem::refused(self.line(), message)
    }

    fn expected(&self, wanted: &str) -> Problem {
        self.refuse(format!("expected {wanted}, found {}", self.found()))
    }

    fn expect(&mut self, kind: &TokenKind, wanted: &str) -> Result<(), Problem> {
        if self.eat(kind) {
            Ok(())
        } else {
            Err(self.expected(wanted))
        }
    }

    fn expect_word(&mut self, word: &str) -> Result<(), Problem> {
        if self.eat_word(word) {
            Ok(())
        } else {
            Err(self.expected(&format!("`{word}`")))
        }
    }

    fn expect_semicolon(&mut self) -> Result<(), Problem> {
        if self.eat(&TokenKind::Semicolon) {
            return Ok(());
        }
        // The `;` is missing from the end of the previous token's line, not from wherever the next token is.
        let line = self
            .tokens
            .get(self.position.saturating_sub(1))
            .map_or_else(|| self.line(), |token| token.line);
        Err(Problem::refused(
            line,
            "this statement doesn't end with `;` (D29)",
        ))
    }

    fn take_word(&mut self, wanted: &str) -> Result<String, Problem> {
        if let Some(TokenKind::Word(word)) = self.kind() {
            let word = word.clone();
            self.advance();
            Ok(word)
        } else {
            Err(self.expected(wanted))
        }
    }

    fn take_value_name(&mut self, wanted: &str) -> Result<String, Problem> {
        if let Some(TokenKind::ValueName(name)) = self.kind() {
            let name = name.clone();
            self.advance();
            Ok(name)
        } else {
            Err(self.expected(wanted))
        }
    }
}

impl Parser {
    fn program(&mut self) -> Result<Program, Problem> {
        let mut functions = Vec::new();
        let mut main = None;
        while self.current().is_some() {
            if self.at_word("fn") {
                let function = self.function()?;
                if !function.is_main {
                    functions.push(function);
                } else if main.is_some() {
                    return Err(Problem::refused(
                        function.line,
                        "a program has exactly one `fn.main` (D101)",
                    ));
                } else {
                    main = Some(function);
                }
            } else if self.at_word("main") {
                return Err(self
                    .refuse("the program starts at `fn.main [] -> void:` … `end fn.main` (D101)"));
            } else if self.at_word("struct") || self.at_word("union") {
                return Err(Problem::not_built(
                    self.line(),
                    "`struct` and `union.tagged` are in the oracle's second slice",
                ));
            } else {
                return Err(self.refuse(format!(
                    "{} can't start a top-level item; the oracle reads `fn` here",
                    self.found()
                )));
            }
        }
        let main = main.ok_or_else(|| {
            Problem::refused(
                1,
                "a program needs `fn.main [] -> void:` … `end fn.main` (D101)",
            )
        })?;
        Ok(Program { functions, main })
    }

    /// Reads `end fn.main` (D101).
    fn end_main_line(&mut self) -> Result<(), Problem> {
        let line = self.line();
        let matched = self.eat_word("end")
            && self.eat_word("fn")
            && self.eat(&TokenKind::Dot)
            && self.eat_word("main");
        if !matched || self.current().is_some_and(|token| token.line == line) {
            return Err(Problem::refused(
                line,
                "this `end` should be `end fn.main` (D13, D101)",
            ));
        }
        Ok(())
    }

    /// Reads `end kind` or `end kind name`, which must match its opener exactly (D13).
    fn end_line(&mut self, kind: &str, name: Option<&str>) -> Result<(), Problem> {
        let line = self.line();
        let wanted = name.map_or_else(
            || format!("`end {kind}`"),
            |name| format!("`end {kind} {name}`"),
        );
        let mismatch = || Problem::refused(line, format!("this `end` should be {wanted} (D13)"));
        if !self.eat_word("end") {
            return Err(self.expected(&format!("{wanted} (D13)")));
        }
        if !self.eat_word(kind) {
            return Err(mismatch());
        }
        if let Some(name) = name
            && !self.eat_word(name)
        {
            return Err(mismatch());
        }
        if self.current().is_some_and(|token| token.line == line) {
            return Err(mismatch());
        }
        Ok(())
    }

    /// Reads statements up to the `end` or `else` that closes the block.
    fn statements(&mut self) -> Result<Vec<Statement>, Problem> {
        let mut statements = Vec::new();
        while !self.at_word("end") && !self.at_word("else") {
            if self.current().is_none() {
                return Err(self.refuse("this block never reaches its `end` (D13)"));
            }
            statements.push(self.statement()?);
        }
        Ok(statements)
    }

    fn statement(&mut self) -> Result<Statement, Problem> {
        let line = self.line();
        let word = match self.kind() {
            Some(TokenKind::Word(word)) => word.clone(),
            Some(TokenKind::ValueName(_)) => return self.assignment(line),
            _ => return Err(self.refuse(format!("{} doesn't start a statement", self.found()))),
        };
        let kind = match word.as_str() {
            "var" | "const" => self.declaration()?,
            "if" => self.if_chain()?,
            "for" => self.for_loop()?,
            "while" => self.while_loop()?,
            "forever" => self.forever_loop()?,
            "break" => self.break_statement()?,
            "return" => self.return_statement()?,
            "std" | "my" => StatementKind::Call(self.call_statement()?),
            _ => {
                if let Some(problem) = self.misplaced_statement(&word) {
                    return Err(problem);
                }
                if self.kind_after(1) != Some(&TokenKind::OpenParen) {
                    return Err(self.refuse(format!("`{word}` doesn't start a statement")));
                }
                StatementKind::Call(self.call_statement()?)
            }
        };
        Ok(Statement { line, kind })
    }

    /// Words that can't start a statement here, and why.
    fn misplaced_statement(&self, word: &str) -> Option<Problem> {
        let line = self.line();
        let problem = match word {
            "match" => Problem::not_built(line, "`match` is in the oracle's second slice (D72)"),
            "shield" => Problem::not_decided(line, "how a shield is written is open (D6)"),
            "fails" => Problem::not_decided(
                line,
                "the `fails … exactly when` wording is a placeholder (D56)",
            ),
            "invariant" | "assert" => Problem::not_decided(
                line,
                "the spelling of loop invariants and asserts is open (D4)",
            ),
            "requires" | "ensures" | "labels" | "defaults" => Problem::refused(
                line,
                format!(
                    "`{word}` belongs at the top of a function's body, before any other line (D53, D54)"
                ),
            ),
            "decreases" => Problem::refused(
                line,
                "`decreases` belongs at the top of a `while` loop's body (D75)",
            ),
            "else" | "end" => {
                Problem::refused(line, format!("this `{word}` has no block to close (D13)"))
            }
            _ => return None,
        };
        Some(problem)
    }
}

impl Parser {
    fn declaration(&mut self) -> Result<StatementKind, Problem> {
        let line = self.line();
        let holder = if self.eat_word("const") {
            Holder::Const
        } else {
            self.expect_word("var")?;
            if self.eat(&TokenKind::Dot) {
                match self.take_word("`immut` or `mut` after `var.`")?.as_str() {
                    "immut" => Holder::VarImmut,
                    "mut" => Holder::VarMut,
                    other => {
                        return Err(Problem::refused(
                            line,
                            format!(
                                "`var.{other}` isn't a kind of value: there are `var`, `var.immut`, `var.mut` and `const` (D30)"
                            ),
                        ));
                    }
                }
            } else {
                Holder::Var
            }
        };
        let name = self.take_value_name("a value's name, starting with `$` (D31)")?;
        // trace D108: every value states its type, or `infer`.
        if !self.eat(&TokenKind::Colon) {
            return Err(
                self.refuse("a value states its type, or `: infer` to have it worked out (D108)")
            );
        }
        let type_spec = if self.eat_word("infer") {
            None
        } else {
            Some(self.type_spec()?)
        };
        if self.at(&TokenKind::Semicolon) {
            return Err(
                self.not_decided_here("declaring a value without a starting value isn't decided")
            );
        }
        self.expect(&TokenKind::Equals, "`=` and a starting value (D41)")?;
        let value = self.expression()?;
        self.expect_semicolon()?;
        Ok(StatementKind::Declare {
            holder,
            name,
            type_spec,
            value,
        })
    }

    fn not_decided_here(&self, message: &str) -> Problem {
        Problem::not_decided(self.line(), message)
    }

    fn assignment(&mut self, line: usize) -> Result<Statement, Problem> {
        let name = self.take_value_name("a value's name")?;
        self.expect(&TokenKind::Equals, "`=`: it sets a value (D41)")?;
        let value = self.expression()?;
        self.expect_semicolon()?;
        Ok(Statement {
            line,
            kind: StatementKind::Assign { name, value },
        })
    }

    fn if_chain(&mut self) -> Result<StatementKind, Problem> {
        self.expect_word("if")?;
        let mut branches = vec![self.branch()?];
        let mut else_body = None;
        while self.eat_word("else") {
            if self.eat_word("if") {
                branches.push(self.branch()?);
            } else {
                self.expect(&TokenKind::Colon, "`:` after `else` (D71)")?;
                else_body = Some(self.statements()?);
                break;
            }
        }
        self.end_line("if", None)?;
        Ok(StatementKind::If {
            branches,
            else_body,
        })
    }

    fn branch(&mut self) -> Result<Branch, Problem> {
        let condition = self.expression()?;
        self.expect(&TokenKind::Colon, "`:` after the condition (D13)")?;
        let body = self.statements()?;
        Ok(Branch { condition, body })
    }

    fn for_loop(&mut self) -> Result<StatementKind, Problem> {
        self.expect_word("for")?;
        let loop_name = if self.at_any_word() {
            Some(self.take_word("a loop name")?)
        } else {
            None
        };
        let variable = self.take_value_name("the loop's value, starting with `$` (D74)")?;
        self.expect_word("in")?;
        if matches!(self.kind(), Some(TokenKind::ValueName(_))) {
            return Err(Problem::not_built(
                self.line(),
                "going through a collection (D74) is in a later slice",
            ));
        }
        let range = self.loop_range()?;
        self.expect(&TokenKind::Colon, "`:` after the range (D13)")?;
        let body = self.statements()?;
        self.end_line("for", loop_name.as_deref())?;
        Ok(StatementKind::For {
            loop_name,
            variable,
            range,
            body,
        })
    }

    fn loop_range(&mut self) -> Result<LoopRange, Problem> {
        let range_line = self.line();
        self.expect_word("range")?;
        self.expect(&TokenKind::OpenBracket, "`[` after `range` (D74)")?;
        let mut from = None;
        let mut end = None;
        let mut step_by = None;
        let mut last_place = 0_u8;
        loop {
            let line = self.line();
            let label = self.take_word("a label: `from:`, `to:`, `before:` or `step_by:` (D73)")?;
            self.expect(&TokenKind::Colon, "`:` after the label (D54)")?;
            let value = self.expression()?;
            let place = match label.as_str() {
                "from" => {
                    from = Some(value);
                    1
                }
                "to" | "before" => {
                    if end.is_some() {
                        return Err(Problem::refused(
                            line,
                            "a range has `to:` or `before:`, not both (D73)",
                        ));
                    }
                    end = Some(if label == "to" {
                        RangeEnd::To(value)
                    } else {
                        RangeEnd::Before(value)
                    });
                    2
                }
                "step_by" => {
                    step_by = Some(value);
                    3
                }
                other => {
                    return Err(Problem::refused(
                        line,
                        format!(
                            "`{other}:` isn't a range label: they are `from:`, `to:`, `before:` and `step_by:` (D73)"
                        ),
                    ));
                }
            };
            if place <= last_place {
                return Err(Problem::refused(
                    line,
                    "range labels go in their declared order: `from:`, then `to:` or `before:`, then `step_by:` (D54)",
                ));
            }
            last_place = place;
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::CloseBracket, "`]` closing the range")?;
        let from = from.ok_or_else(|| {
            Problem::not_decided(range_line, "a range without `from:` isn't decided")
        })?;
        let end = end.ok_or_else(|| {
            Problem::refused(range_line, "a range needs `to:` or `before:` (D73)")
        })?;
        Ok(LoopRange { from, end, step_by })
    }

    fn while_loop(&mut self) -> Result<StatementKind, Problem> {
        self.expect_word("while")?;
        let condition = self.expression()?;
        self.expect(&TokenKind::Colon, "`:` after the condition (D13)")?;
        let decreases = if self.eat_word("decreases") {
            let measure = self.expression()?;
            self.expect_semicolon()?;
            Some(measure)
        } else {
            None
        };
        let body = self.statements()?;
        self.end_line("while", None)?;
        Ok(StatementKind::While {
            condition,
            decreases,
            body,
        })
    }

    fn forever_loop(&mut self) -> Result<StatementKind, Problem> {
        self.expect_word("forever")?;
        let loop_name = if self.at_any_word() {
            Some(self.take_word("a loop name")?)
        } else {
            None
        };
        self.expect(&TokenKind::Colon, "`:` after `forever` (D76)")?;
        let body = self.statements()?;
        self.end_line("forever", loop_name.as_deref())?;
        Ok(StatementKind::Forever { loop_name, body })
    }

    fn return_statement(&mut self) -> Result<StatementKind, Problem> {
        self.expect_word("return")?;
        if self.at(&TokenKind::Semicolon) {
            return Err(self.refuse(
                "a bare `return;` is refused; a `-> void` function returns with `return void;` (D100)",
            ));
        }
        let value = if self.eat_word("void") {
            None
        } else {
            Some(self.expression()?)
        };
        self.expect_semicolon()?;
        Ok(StatementKind::Return { value })
    }

    fn break_statement(&mut self) -> Result<StatementKind, Problem> {
        self.expect_word("break")?;
        let loop_name = if self.at_any_word() {
            Some(self.take_word("a loop name")?)
        } else {
            None
        };
        self.expect_semicolon()?;
        Ok(StatementKind::Break { loop_name })
    }
}

impl Parser {
    fn call_statement(&mut self) -> Result<Call, Problem> {
        let call = self.call()?;
        self.expect_semicolon()?;
        Ok(call)
    }

    fn call(&mut self) -> Result<Call, Problem> {
        let line = self.line();
        let target = if self.eat_word("std") {
            self.expect(&TokenKind::DoubleColon, "`::` after `std`")?;
            let name = self.take_word("a name from `std`")?;
            if name != "print" {
                return Err(Problem::not_built(
                    line,
                    format!("`std::{name}`: the oracle's `std` has only `print` so far"),
                ));
            }
            if self.at(&TokenKind::Dot) {
                return Err(Problem::not_built(
                    line,
                    "narrowings of `std::print` aren't in the oracle yet (D57)",
                ));
            }
            CallTarget::Print
        } else if self.at_word("my") && self.kind_after(1) == Some(&TokenKind::Dot) {
            self.advance();
            self.advance();
            CallTarget::Own {
                name: self.take_word("a function's name after `my.`")?,
                marked: true,
            }
        } else {
            CallTarget::Own {
                name: self.take_word("a function's name")?,
                marked: false,
            }
        };
        self.expect(
            &TokenKind::OpenParen,
            "`(`: a call runs, so it uses `( )` (D50)",
        )?;
        let mut arguments = Vec::new();
        if !self.eat(&TokenKind::CloseParen) {
            loop {
                arguments.push(self.argument()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::CloseParen, "`)` closing the call")?;
        }
        Ok(Call {
            line,
            target,
            arguments,
        })
    }

    fn argument(&mut self) -> Result<Argument, Problem> {
        let line = self.line();
        let label = if self.at_any_word() && self.kind_after(1) == Some(&TokenKind::Colon) {
            let label = self.take_word("a label")?;
            self.advance();
            Some(label)
        } else {
            None
        };
        if let Some(TokenKind::Word(word)) = self.kind() {
            let followed_by_value = matches!(
                self.kind_after(1),
                Some(TokenKind::ValueName(_) | TokenKind::Dot)
            );
            match word.as_str() {
                "Give" | "Copy" | "Lend" => {
                    return Err(self.refuse(
                        "the oracle's first slice has only integers and `bool`, which are always copied and take no ownership word (D92)",
                    ));
                }
                "give" | "copy" | "lend" if followed_by_value => {
                    return Err(self.refuse(
                        "ownership words start with a capital: `Give`, `Copy`, `Lend` (D84)",
                    ));
                }
                _ => {}
            }
        }
        let value = self.expression()?;
        Ok(Argument { line, label, value })
    }
}

impl Parser {
    fn function(&mut self) -> Result<Function, Problem> {
        let line = self.line();
        self.expect_word("fn")?;
        let narrowing = if self.eat(&TokenKind::Dot) {
            Some(self.take_word("`pure` or `main` after `fn.`")?)
        } else {
            None
        };
        let (pure, is_main) = match narrowing.as_deref() {
            None => (false, false),
            Some("pure") => (true, false),
            Some("main") => (false, true),
            Some(other) => {
                return Err(Problem::refused(
                    line,
                    format!(
                        "`fn.{other}` isn't a kind of function: there are `fn`, `fn.pure` (D51) and `fn.main` (D101)"
                    ),
                ));
            }
        };
        if self.at(&TokenKind::Dot) {
            return Err(self.refuse("a function takes one narrowing: `fn.pure` or `fn.main`"));
        }
        let name = if is_main {
            "fn.main".to_owned()
        } else {
            self.take_word("the function's name")?
        };
        self.expect(
            &TokenKind::OpenBracket,
            "`[` before the parameters: a definition describes, so it uses `[ ]` (D50)",
        )?;
        let mut parameters = Vec::new();
        if !self.eat(&TokenKind::CloseBracket) {
            loop {
                parameters.push(self.parameter()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::CloseBracket, "`]` after the parameters")?;
        }
        if !self.eat(&TokenKind::Arrow) {
            return Err(self.refuse(
                "every function states its result after `->`; one that gives nothing back says `-> void` (D98, D99)",
            ));
        }
        let result = if self.eat_word("void") {
            None
        } else {
            Some(self.type_spec()?)
        };
        self.expect(&TokenKind::Colon, "`:` ending the function's header (D13)")?;
        let top = self.body_top()?;
        let body = self.statements()?;
        if is_main {
            self.end_main_line()?;
        } else {
            self.end_line("fn", Some(&name))?;
        }
        Ok(Function {
            name,
            line,
            pure,
            is_main,
            parameters,
            result,
            labels: top.labels,
            defaults: top.defaults,
            requires: top.requires,
            ensures: top.ensures,
            body,
        })
    }

    fn parameter(&mut self) -> Result<Parameter, Problem> {
        let line = self.line();
        let changeability = if self.eat_word("var") {
            if self.eat(&TokenKind::Dot) {
                match self.take_word("`mut` after `var.`")?.as_str() {
                    "mut" => Changeability::MustChange,
                    "immut" => {
                        return Err(Problem::refused(
                            line,
                            "a parameter is `var.immut` already; write it bare (D31, D39)",
                        ));
                    }
                    other => {
                        return Err(Problem::refused(
                            line,
                            format!("`var.{other}` isn't a kind of parameter (D31)"),
                        ));
                    }
                }
            } else {
                Changeability::Free
            }
        } else {
            Changeability::Fixed
        };
        let name = self.take_value_name("a parameter's name, starting with `$` (D31)")?;
        self.expect(&TokenKind::Colon, "`:` and the parameter's type")?;
        self.refuse_parameter_ownership()?;
        let type_spec = self.type_spec()?;
        Ok(Parameter {
            name,
            line,
            changeability,
            type_spec,
        })
    }

    fn refuse_parameter_ownership(&self) -> Result<(), Problem> {
        let Some(TokenKind::Word(word)) = self.kind() else {
            return Ok(());
        };
        let message = match word.as_str() {
            "Lent" if self.kind_after(1) != Some(&TokenKind::Dot) => {
                "`Lent` is the default for a parameter; write the bare parameter (D92)"
            }
            "Lent" | "Given" => {
                "the oracle's first slice has only integers and `bool`, which are always copied and take no ownership word (D92)"
            }
            "Lend" | "Give" | "Copy" => {
                "parameters say `Lent.mut` or `Given`; `Lend`, `Give` and `Copy` are for calls (D91)"
            }
            "lent" | "given" | "lend" | "give" | "copy" => {
                "ownership words start with a capital (D84)"
            }
            _ => return Ok(()),
        };
        Err(self.refuse(message))
    }

    fn body_top(&mut self) -> Result<BodyTop, Problem> {
        let mut top = BodyTop {
            labels: None,
            defaults: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        };
        let mut stage = 0_u8;
        loop {
            let line = self.line();
            let next_stage = if self.at_word("labels") {
                1
            } else if self.at_word("defaults") {
                2
            } else if self.at_word("requires") || self.at_word("ensures") {
                3
            } else if self.at_word("fails") {
                return Err(Problem::not_decided(
                    line,
                    "the `fails … exactly when` wording is a placeholder (D56)",
                ));
            } else if self.at_word("decreases") {
                return Err(Problem::not_built(
                    line,
                    "`decreases` on a function (D4, D7) is in a later slice",
                ));
            } else {
                break;
            };
            if next_stage < stage || (next_stage == stage && next_stage < 3) {
                return Err(Problem::refused(
                    line,
                    "the top of a body is `labels`, then `defaults`, then contracts, each line once (D53, D54)",
                ));
            }
            stage = next_stage;
            match next_stage {
                1 => top.labels = Some(self.labels_line()?),
                2 => top.defaults = self.defaults_line()?,
                _ => {
                    let is_requires = self.at_word("requires");
                    self.advance();
                    let condition = self.expression()?;
                    self.expect_semicolon()?;
                    if is_requires {
                        top.requires.push(condition);
                    } else {
                        top.ensures.push(condition);
                    }
                }
            }
        }
        Ok(top)
    }

    fn labels_line(&mut self) -> Result<Labels, Problem> {
        let line = self.line();
        self.expect_word("labels")?;
        let any_order = if self.eat(&TokenKind::Dot) {
            match self.take_word("`any_order` after `labels.`")?.as_str() {
                "any_order" => true,
                "in_order" => {
                    return Err(Problem::refused(
                        line,
                        "`labels.in_order` spells out the default; write `labels` (D39, D54)",
                    ));
                }
                other => {
                    return Err(Problem::refused(
                        line,
                        format!("`labels.{other}` isn't a kind of labels line (D54)"),
                    ));
                }
            }
        } else {
            false
        };
        self.expect(&TokenKind::OpenBracket, "`[` after `labels` (D54)")?;
        let mut entries = Vec::new();
        loop {
            let parameter = self.take_value_name("a parameter's `$name`")?;
            self.expect_word("as")?;
            let label = self.take_word("the label callers write")?;
            entries.push(LabelEntry { parameter, label });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::CloseBracket, "`]` closing the labels")?;
        self.expect_semicolon()?;
        Ok(Labels {
            line,
            any_order,
            entries,
        })
    }

    fn defaults_line(&mut self) -> Result<Vec<ParameterDefault>, Problem> {
        self.expect_word("defaults")?;
        if self.at(&TokenKind::Dot) {
            return Err(Problem::not_built(
                self.line(),
                "`defaults.fixed` (D57) is for function collections, in a later slice",
            ));
        }
        self.expect(&TokenKind::OpenBracket, "`[` after `defaults` (D54)")?;
        let mut defaults = Vec::new();
        loop {
            let line = self.line();
            let parameter = self.take_value_name("a parameter's `$name`")?;
            self.expect(&TokenKind::Equals, "`=` and the default value (D54)")?;
            let value = self.expression()?;
            defaults.push(ParameterDefault {
                line,
                parameter,
                value,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::CloseBracket, "`]` closing the defaults")?;
        self.expect_semicolon()?;
        Ok(defaults)
    }

    fn type_spec(&mut self) -> Result<TypeSpec, Problem> {
        let line = self.line();
        let name = self.take_word("a type")?;
        match name.as_str() {
            "bool" => Ok(TypeSpec::Bool),
            "int" => self.integer_type(line),
            "infer" => Err(Problem::refused(
                line,
                "`infer` is for local values only; parameters and results write their types (D14, D108)",
            )),
            "string" => Err(Problem::not_decided(line, "text types aren't designed yet")),
            "void" => Err(Problem::refused(
                line,
                "`void` is only a function's result (`-> void`, D99), not a type for a value",
            )),
            "str" => Err(Problem::refused(
                line,
                "`str` is written in full: `string` (D94)",
            )),
            other => Err(Problem::not_built(
                line,
                format!("`{other}`: the oracle's first slice has only `int[…]` and `bool`"),
            )),
        }
    }

    /// `int[…]`, or a narrowed `int.signed.N_bit[…]` / `int.unsigned.N_bit[…]` (D14, D26, D27, D105).
    fn integer_type(&mut self, line: usize) -> Result<TypeSpec, Problem> {
        // `None` is bare `int`, `Some(None)` is `int.wide`, `Some(Some(…))` an exact width.
        let width = if !self.eat(&TokenKind::Dot) {
            None
        } else if self.eat_word("wide") {
            Some(None)
        } else {
            Some(Some(self.integer_width(line)?))
        };
        if !self.eat(&TokenKind::OpenBracket) {
            return Err(Problem::not_decided(
                line,
                "an integer type without bounds is open (D26); give its bounds: `int[min: …, max: …]`",
            ));
        }
        if self.at_word("max") {
            return Err(Problem::not_decided(
                line,
                "whether `max:` may come before `min:` isn't decided",
            ));
        }
        self.expect_word("min")?;
        self.expect(&TokenKind::Colon, "`:` after `min` (D54)")?;
        let min = self.bound()?;
        self.expect(&TokenKind::Comma, "`,` and then `max:`")?;
        self.expect_word("max")?;
        self.expect(&TokenKind::Colon, "`:` after `max` (D54)")?;
        let max = self.bound()?;
        self.expect(&TokenKind::CloseBracket, "`]` closing the bounds")?;
        let bounds = Bounds { min, max };
        match width {
            Some(Some((signed, bits))) => check_width(line, &bounds, signed, bits)?,
            // trace D107: `int.wide` is only for bounds that need more than 64 bits.
            Some(None) if fits_64_bits(&bounds) => {
                return Err(Problem::refused(
                    line,
                    "these bounds fit in 64 bits, so `int.wide` isn't needed; write `int[…]` (D107)",
                ));
            }
            // trace D105: a bare `int` holds at most 64 bits; wider ones say so.
            None if !fits_64_bits(&bounds) => {
                return Err(Problem::refused(
                    line,
                    "these bounds need more than 64 bits; write `int.wide[…]` or an exact width such as `int.unsigned.128_bit[…]` (D105, D107)",
                ));
            }
            Some(None) | None => {}
        }
        Ok(TypeSpec::Integer(bounds))
    }

    /// Reads `signed.N_bit` or `unsigned.N_bit` after `int.`.
    fn integer_width(&mut self, line: usize) -> Result<(bool, u32), Problem> {
        let signed = match self
            .take_word("`signed` or `unsigned` after `int.`")?
            .as_str()
        {
            "signed" => true,
            "unsigned" => false,
            other => {
                return Err(Problem::refused(
                    line,
                    format!(
                        "`int.{other}` isn't a narrowing of `int`: there are `int.wide`, `int.signed` and `int.unsigned` (D26, D107)"
                    ),
                ));
            }
        };
        if !self.eat(&TokenKind::Dot) {
            return Err(Problem::not_decided(
                line,
                "a signedness without a width (`int.signed[…]`) isn't decided; write e.g. `int.signed.32_bit[…]`",
            ));
        }
        let width = self.take_word("a width such as `32_bit`")?;
        let bits = width
            .strip_suffix("_bit")
            .and_then(|digits| digits.parse::<u32>().ok())
            .filter(|bits| *bits > 0)
            .ok_or_else(|| {
                Problem::refused(
                    line,
                    format!("`{width}` isn't a width; widths are written like `32_bit` (D26)"),
                )
            })?;
        Ok((signed, bits))
    }

    fn bound(&mut self) -> Result<BigInt, Problem> {
        let negative = self.eat(&TokenKind::Minus);
        let Some(TokenKind::Integer(number)) = self.kind() else {
            return Err(Problem::not_built(
                self.line(),
                "bounds written with named constants are in a later slice",
            ));
        };
        let number = number.clone();
        self.advance();
        Ok(if negative { -number } else { number })
    }
}

// Operator strength is a provisional reading (oracle/README.md): `x`, `div`, `mod` over `+`, `-`; then comparisons;
// then `not`, `and`, `or`.
impl Parser {
    fn expression(&mut self) -> Result<Expression, Problem> {
        self.or_expression()
    }

    fn or_expression(&mut self) -> Result<Expression, Problem> {
        let mut left = self.and_expression()?;
        while self.at_word("or") {
            let line = self.line();
            self.advance();
            self.refuse_logic_narrowing()?;
            let right = self.and_expression()?;
            left = binary(line, Operator::Or, left, right);
        }
        Ok(left)
    }

    fn and_expression(&mut self) -> Result<Expression, Problem> {
        let mut left = self.not_expression()?;
        while self.at_word("and") {
            let line = self.line();
            self.advance();
            self.refuse_logic_narrowing()?;
            let right = self.not_expression()?;
            left = binary(line, Operator::And, left, right);
        }
        Ok(left)
    }

    fn refuse_logic_narrowing(&self) -> Result<(), Problem> {
        if self.at(&TokenKind::Dot) {
            Err(Problem::not_decided(
                self.line(),
                "the narrowing of `and`/`or` that evaluates both sides isn't spelled yet (D40)",
            ))
        } else {
            Ok(())
        }
    }

    fn not_expression(&mut self) -> Result<Expression, Problem> {
        if self.at_word("not") {
            let line = self.line();
            self.advance();
            let operand = self.not_expression()?;
            return Ok(Expression {
                line,
                kind: ExpressionKind::Not(Box::new(operand)),
            });
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Expression, Problem> {
        let left = self.additive()?;
        let Some(operator) = self.comparison_operator() else {
            return Ok(left);
        };
        let line = self.line();
        self.advance();
        let right = self.additive()?;
        if self.comparison_operator().is_some() {
            return Err(Problem::not_decided(
                self.line(),
                "chained comparisons such as `1 < $x < 5` aren't decided",
            ));
        }
        Ok(binary(line, operator, left, right))
    }

    fn comparison_operator(&self) -> Option<Operator> {
        match self.kind()? {
            TokenKind::Less => Some(Operator::Less),
            TokenKind::Greater => Some(Operator::Greater),
            TokenKind::Word(word) => match word.as_str() {
                "at_most" => Some(Operator::AtMost),
                "at_least" => Some(Operator::AtLeast),
                "equal" => Some(Operator::Equal),
                "not_equal" => Some(Operator::NotEqual),
                _ => None,
            },
            _ => None,
        }
    }

    fn additive(&mut self) -> Result<Expression, Problem> {
        let mut left = self.multiplicative()?;
        loop {
            let operator = match self.kind() {
                Some(TokenKind::Plus) => Operator::Add,
                Some(TokenKind::Minus) => Operator::Subtract,
                _ => return Ok(left),
            };
            let line = self.line();
            self.advance();
            let right = self.multiplicative()?;
            left = binary(line, operator, left, right);
        }
    }

    fn multiplicative(&mut self) -> Result<Expression, Problem> {
        let mut left = self.unary()?;
        loop {
            let line = self.line();
            let operator = if self.eat_word("x") {
                Operator::Multiply
            } else if self.eat_word("div") {
                Operator::Divide(self.rounding()?)
            } else if self.eat_word("mod") {
                Operator::Remainder(self.remainder_rule()?)
            } else {
                return Ok(left);
            };
            let right = self.unary()?;
            left = binary(line, operator, left, right);
        }
    }

    fn rounding(&mut self) -> Result<Rounding, Problem> {
        if !self.eat(&TokenKind::Dot) {
            return Ok(Rounding::Exact);
        }
        let line = self.line();
        match self
            .take_word("`floor`, `trunc` or `ceil` after `div.`")?
            .as_str()
        {
            "floor" => Ok(Rounding::Floor),
            "trunc" => Ok(Rounding::Trunc),
            "ceil" => Ok(Rounding::Ceil),
            "exact" => Err(Problem::refused(
                line,
                "`div.exact` spells out the default; write `div` (D37, D39)",
            )),
            other => Err(Problem::refused(
                line,
                format!(
                    "`div.{other}` isn't a division: there are `div`, `div.floor`, `div.trunc`, `div.ceil` (D37)"
                ),
            )),
        }
    }

    fn remainder_rule(&mut self) -> Result<RemainderRule, Problem> {
        if !self.eat(&TokenKind::Dot) {
            return Ok(RemainderRule::NonNegative);
        }
        let line = self.line();
        match self
            .take_word("`floor`, `trunc` or `euclid` after `mod.`")?
            .as_str()
        {
            "floor" => Ok(RemainderRule::Floor),
            "trunc" => Ok(RemainderRule::Trunc),
            "euclid" => Ok(RemainderRule::Euclid),
            other => Err(Problem::refused(
                line,
                format!(
                    "`mod.{other}` isn't a remainder: there are `mod`, `mod.floor`, `mod.trunc`, `mod.euclid` (D38)"
                ),
            )),
        }
    }

    fn unary(&mut self) -> Result<Expression, Problem> {
        if self.at(&TokenKind::Minus) {
            let line = self.line();
            self.advance();
            let operand = self.unary()?;
            return Ok(Expression {
                line,
                kind: ExpressionKind::Negate(Box::new(operand)),
            });
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expression, Problem> {
        let line = self.line();
        let Some(kind) = self.kind().cloned() else {
            return Err(self.refuse("the program ends in the middle of a value"));
        };
        let expression_kind = match kind {
            TokenKind::Integer(number) => ExpressionKind::Integer(number),
            TokenKind::Text(text) => ExpressionKind::Text(text),
            TokenKind::ValueName(name) => ExpressionKind::Value(name),
            TokenKind::OpenParen => {
                // trace D50: maths grouping is the one place `( )` runs nothing.
                self.advance();
                let inner = self.expression()?;
                self.expect(&TokenKind::CloseParen, "`)` closing the group")?;
                return Ok(inner);
            }
            TokenKind::Word(word) => return self.word_value(line, &word),
            other => {
                return Err(self.refuse(format!("{} can't start a value here", describe(&other))));
            }
        };
        self.advance();
        Ok(Expression {
            line,
            kind: expression_kind,
        })
    }

    fn word_value(&mut self, line: usize, word: &str) -> Result<Expression, Problem> {
        let kind = match word {
            "true" => ExpressionKind::Bool(true),
            "false" => ExpressionKind::Bool(false),
            "newline" => ExpressionKind::Newline,
            "newln" => {
                return Err(Problem::refused(
                    line,
                    "`newln` is written in full: `newline` (D94)",
                ));
            }
            "old" => {
                self.advance();
                self.expect(&TokenKind::OpenBracket, "`[` after `old` (D53)")?;
                let name = self.take_value_name("a parameter's `$name`")?;
                self.expect(&TokenKind::CloseBracket, "`]` closing `old[…]`")?;
                return Ok(Expression {
                    line,
                    kind: ExpressionKind::Old(name),
                });
            }
            "result" => ExpressionKind::Result,
            "std" => {
                return Err(Problem::not_built(
                    line,
                    "`std` inside a value is in a later slice",
                ));
            }
            "my" if self.kind_after(1) == Some(&TokenKind::Dot) => {
                return Ok(Expression {
                    line,
                    kind: ExpressionKind::Call(self.call()?),
                });
            }
            _ if self.kind_after(1) == Some(&TokenKind::OpenParen) => {
                return Ok(Expression {
                    line,
                    kind: ExpressionKind::Call(self.call()?),
                });
            }
            _ => {
                return Err(Problem::refused(
                    line,
                    format!("`{word}` can't start a value here"),
                ));
            }
        };
        self.advance();
        Ok(Expression { line, kind })
    }
}

/// Whether bounds fit in 64 bits, signed or unsigned (D27, D105).
fn fits_64_bits(bounds: &Bounds) -> bool {
    let unsigned_fits =
        bounds.min.sign() != Sign::Minus && bounds.max < (BigInt::from(1) << 64_u32);
    let signed_fits =
        bounds.min >= -(BigInt::from(1) << 63_u32) && bounds.max < (BigInt::from(1) << 63_u32);
    unsigned_fits || signed_fits
}

/// Refuses bounds that a narrowed width can't hold (D27).
fn check_width(line: usize, bounds: &Bounds, signed: bool, bits: u32) -> Result<(), Problem> {
    let (lowest, highest) = if signed {
        let half = BigInt::from(1) << bits.saturating_sub(1);
        (-half.clone(), half - 1)
    } else {
        (BigInt::from(0), (BigInt::from(1) << bits) - 1)
    };
    if bounds.min < lowest || bounds.max > highest {
        let kind = if signed { "signed" } else { "unsigned" };
        return Err(Problem::refused(
            line,
            format!(
                "`int.{kind}.{bits}_bit` holds {lowest} to {highest}, which these bounds don't fit in (D27)"
            ),
        ));
    }
    Ok(())
}

fn binary(line: usize, operator: Operator, left: Expression, right: Expression) -> Expression {
    Expression {
        line,
        kind: ExpressionKind::Binary {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        },
    }
}
// end section oracle.parser
