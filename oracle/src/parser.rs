// section oracle.parser:
//! Builds the program's tree from tokens: blocks and `end` (D13), statements and `;` (D29), functions (D50–D54),
//! control flow (D71–D77) and expressions (D34–D40). Every node keeps its place, for messages (D117, D118).

use num_bigint::{BigInt, Sign};

use crate::ast::{
    Argument, Bounds, Branch, Call, CallTarget, Case, CaseBinding, Changeability, Expression,
    ExpressionKind, Field, FieldValue, Function, Handoff, Holder, Holding, LabelEntry, Labels,
    LoopRange, MatchCase, Operator, Parameter, ParameterDefault, Program, RangeEnd, RemainderRule,
    Rounding, Statement, StatementKind, StructDefinition, Target, TypeSpec, UnionDefinition,
};
use crate::lexer::{Token, TokenKind, describe};
use crate::outcome::{Place, Problem};
use crate::rules::is_radium_word;

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

/// The place used when a program is empty.
const START: Place = Place {
    line: 1,
    byte_column: 1,
    column: 1,
    width: 1,
};

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

    /// The current token's place, or the last token's at the end of the program.
    fn place(&self) -> Place {
        self.current()
            .or_else(|| self.tokens.last())
            .map_or(START, |token| token.place)
    }

    /// The place of the token just read.
    fn previous(&self) -> Place {
        self.position
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map_or_else(|| self.place(), |token| token.place)
    }

    /// From `start` to the end of the token just read.
    fn span_from(&self, start: Place) -> Place {
        start.to(self.previous())
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

    /// "Expected …, found …" at the current token.
    fn expected(&self, wanted: &str, why: &str) -> Problem {
        Problem::refused(
            self.place(),
            format!("expected {wanted}, found {}", self.found()),
            why,
        )
    }

    fn expect(&mut self, kind: &TokenKind, wanted: &str, why: &str) -> Result<(), Problem> {
        if self.eat(kind) {
            Ok(())
        } else {
            Err(self.expected(wanted, why))
        }
    }

    fn expect_word(&mut self, word: &str, why: &str) -> Result<(), Problem> {
        if self.eat_word(word) {
            Ok(())
        } else {
            Err(self.expected(&format!("`{word}`"), why))
        }
    }

    fn expect_semicolon(&mut self) -> Result<(), Problem> {
        if self.eat(&TokenKind::Semicolon) {
            return Ok(());
        }
        // The `;` is missing from the end of the token just read, not from wherever the next token is.
        Err(Problem::refused(
            self.previous(),
            "this statement doesn't end with `;`",
            "every statement ends with `;`; a line break never ends one (D29)",
        )
        .fix("add `;` after this"))
    }

    fn take_word(&mut self, wanted: &str, why: &str) -> Result<String, Problem> {
        if let Some(TokenKind::Word(word)) = self.kind() {
            let word = word.clone();
            self.advance();
            Ok(word)
        } else {
            Err(self.expected(wanted, why))
        }
    }

    fn take_value_name(&mut self, wanted: &str) -> Result<String, Problem> {
        if let Some(TokenKind::ValueName(name)) = self.kind() {
            let name = name.clone();
            self.advance();
            Ok(name)
        } else {
            Err(self.expected(wanted, "a value's name starts with `$` (D31)"))
        }
    }
}

impl Parser {
    fn program(&mut self) -> Result<Program, Problem> {
        let mut functions = Vec::new();
        let mut structs = Vec::new();
        let mut unions = Vec::new();
        let mut main: Option<Function> = None;
        while self.current().is_some() {
            if self.at_word("struct") {
                structs.push(self.struct_definition()?);
            } else if self.at_word("union") {
                unions.push(self.union_definition()?);
            } else if self.at_word("fn") {
                let function = self.function()?;
                if !function.is_main {
                    functions.push(function);
                } else if let Some(first) = &main {
                    return Err(Problem::refused(
                        function.place,
                        "a second `fn.main`",
                        "a program has exactly one `fn.main`, where it starts (D101)",
                    )
                    .because(first.place, "the program's `fn.main` is here"));
                } else {
                    main = Some(function);
                }
            } else if self.at_word("main") {
                return Err(Problem::refused(
                    self.place(),
                    "`main:` is no longer how a program starts",
                    "the program starts at `fn.main`, a function nothing may call (D101)",
                )
                .fix("`fn.main [] -> void:` … `end fn.main`"));
            } else {
                return Err(Problem::refused(
                    self.place(),
                    format!("{} can't start a top-level item", self.found()),
                    "the top level holds functions and types: `fn`, `struct`, `union.tagged` (D51, D58, D59)",
                ));
            }
        }
        let main = main.ok_or_else(|| {
            Problem::refused(
                START,
                "this program has no `fn.main`",
                "a program starts at its one `fn.main [] -> void:` … `end fn.main` (D101)",
            )
        })?;
        Ok(Program {
            structs,
            unions,
            functions,
            main,
        })
    }

    /// Reads `end fn.main` (D101).
    fn end_main_line(&mut self, opener: Place) -> Result<(), Problem> {
        let start = self.place();
        let matched = self.eat_word("end")
            && self.eat_word("fn")
            && self.eat(&TokenKind::Dot)
            && self.eat_word("main");
        if !matched
            || self
                .current()
                .is_some_and(|token| token.place.line == start.line)
        {
            return Err(Problem::refused(
                self.span_from(start),
                "this `end` doesn't match its opener",
                "every `end` matches its opener exactly (D13)",
            )
            .fix("`end fn.main`")
            .because(opener, "the block opens here"));
        }
        Ok(())
    }

    /// Reads `end kind` or `end kind name`, which must match its opener exactly (D13).
    fn end_line(&mut self, kind: &str, name: Option<&str>, opener: Place) -> Result<(), Problem> {
        let start = self.place();
        let wanted = name.map_or_else(
            || format!("`end {kind}`"),
            |name| format!("`end {kind} {name}`"),
        );
        if !self.eat_word("end") {
            return Err(self
                .expected(&wanted, "every block closes with `end` and its kind (D13)")
                .because(opener, "the block opens here"));
        }
        let matched = self.eat_word(kind) && name.is_none_or(|name| self.eat_word(name));
        if !matched
            || self
                .current()
                .is_some_and(|token| token.place.line == start.line)
        {
            // Point at the whole `end` line as written.
            while self
                .current()
                .is_some_and(|token| token.place.line == start.line)
            {
                self.advance();
            }
            return Err(Problem::refused(
                self.span_from(start),
                "this `end` doesn't match its opener",
                "every `end` matches its opener exactly (D13)",
            )
            .fix(wanted)
            .because(opener, "the block opens here"));
        }
        Ok(())
    }

    /// Reads statements up to the `end` or `else` that closes the block.
    fn statements(&mut self, opener: Place) -> Result<Vec<Statement>, Problem> {
        let mut statements = Vec::new();
        while !self.at_word("end") && !self.at_word("else") && !self.at_word("case") {
            if self.current().is_none() {
                return Err(Problem::refused(
                    self.place(),
                    "the program ends inside a block",
                    "every block closes with `end` and its kind (D13)",
                )
                .because(opener, "the block opens here"));
            }
            statements.push(self.statement()?);
        }
        Ok(statements)
    }

    fn statement(&mut self) -> Result<Statement, Problem> {
        let start = self.place();
        let word = match self.kind() {
            Some(TokenKind::Word(word)) => word.clone(),
            Some(TokenKind::ValueName(_)) => return self.assignment(start),
            _ => {
                return Err(Problem::refused(
                    start,
                    format!("{} doesn't start a statement", self.found()),
                    "a statement starts with a keyword, a value's `$name` or a call (SSS: Radium 1.3)",
                ));
            }
        };
        let kind = match word.as_str() {
            "var" | "const" => self.declaration()?,
            "if" => self.if_chain()?,
            "for" => self.for_loop()?,
            "while" => self.while_loop()?,
            "forever" => self.forever_loop()?,
            "break" => self.break_statement()?,
            "match" => self.match_statement()?,
            "return" => self.return_statement()?,
            "std" | "my" => StatementKind::Call(self.call_statement()?),
            _ => {
                if let Some(problem) = self.misplaced_statement(&word) {
                    return Err(problem);
                }
                if self.kind_after(1) != Some(&TokenKind::OpenParen) {
                    return Err(Problem::refused(
                        start,
                        format!("`{word}` doesn't start a statement"),
                        "a statement starts with a keyword, a value's `$name` or a call (SSS: Radium 1.3)",
                    ));
                }
                StatementKind::Call(self.call_statement()?)
            }
        };
        Ok(Statement {
            place: self.span_from(start),
            kind,
        })
    }

    /// Words that can't start a statement here, and why.
    fn misplaced_statement(&self, word: &str) -> Option<Problem> {
        let place = self.place();
        let problem = match word {
            "shield" => Problem::not_decided(place, "how a shield is written (D6)"),
            "fails" => Problem::not_decided(place, "the `fails … exactly when` wording (D56)"),
            "invariant" | "assert" => {
                Problem::not_decided(place, "the spelling of loop invariants and asserts (D4)")
            }
            "requires" | "ensures" | "labels" | "defaults" => Problem::refused(
                place,
                format!("`{word}` after the top of the body"),
                "a body starts with `labels`, then `defaults`, then contracts, before any other line (D53, D54)",
            ),
            "decreases" => Problem::refused(
                place,
                "`decreases` here",
                "`decreases` is the first line of a `while` loop's body (D75)",
            ),
            "case" => Problem::refused(
                place,
                "`case` outside a `match`",
                "a `case` is a branch of a `match` (D72)",
            ),
            "else" | "end" => Problem::refused(
                place,
                format!("this `{word}` has no block to close"),
                "`else` and `end` close a block that's open (D13, D71)",
            ),
            _ => return None,
        };
        Some(problem)
    }
}

impl Parser {
    fn declaration(&mut self) -> Result<StatementKind, Problem> {
        let start = self.place();
        let holder = if self.eat_word("const") {
            Holder::Const
        } else {
            self.expect_word("var", "values are declared with `var` or `const` (D30)")?;
            if self.eat(&TokenKind::Dot) {
                match self
                    .take_word(
                        "`immut` or `mut` after `var.`",
                        "`var` narrows to `var.immut` or `var.mut` (D30)",
                    )?
                    .as_str()
                {
                    "immut" => Holder::VarImmut,
                    "mut" => Holder::VarMut,
                    other => {
                        return Err(Problem::refused(
                            self.span_from(start),
                            format!("`var.{other}` isn't a kind of value"),
                            "values are `var`, `var.immut`, `var.mut` or `const` (D30)",
                        ));
                    }
                }
            } else {
                Holder::Var
            }
        };
        let name = self.take_value_name("a value's name, starting with `$`")?;
        // trace D108: every value states its type, or `infer`.
        if !self.eat(&TokenKind::Colon) {
            return Err(Problem::refused(
                self.previous(),
                format!("`${name}` has no type"),
                "every value states its type, or `: infer` to have it worked out (D108)",
            ));
        }
        let type_spec = if self.eat_word("infer") {
            None
        } else {
            Some(self.type_spec()?)
        };
        if self.at(&TokenKind::Semicolon) {
            return Err(Problem::not_decided(
                self.span_from(start),
                "declaring a value without a starting value",
            ));
        }
        self.expect(
            &TokenKind::Equals,
            "`=` and a starting value",
            "a declaration sets its value with `=` (D41)",
        )?;
        let value = self.expression()?;
        self.expect_semicolon()?;
        Ok(StatementKind::Declare {
            holder,
            name,
            type_spec,
            value,
        })
    }

    fn assignment(&mut self, start: Place) -> Result<Statement, Problem> {
        let name = self.take_value_name("a value's name")?;
        let mut fields = Vec::new();
        while self.at(&TokenKind::Dot) {
            self.advance();
            fields.push(self.take_word("a field's name", "`.` reads a field (D26, D58)")?);
        }
        if self.at(&TokenKind::OpenBracket) {
            return Err(Problem::not_built(
                self.place(),
                "changing an `array` item (D64, D70)",
            ));
        }
        self.expect(
            &TokenKind::Equals,
            "`=`",
            "`=` sets a value; comparing is `equal` (D41)",
        )?;
        let value = self.expression()?;
        self.expect_semicolon()?;
        Ok(Statement {
            place: self.span_from(start),
            kind: StatementKind::Assign {
                target: Target { name, fields },
                value,
            },
        })
    }

    fn if_chain(&mut self) -> Result<StatementKind, Problem> {
        let opener = self.place();
        self.expect_word("if", "SSS: Radium 5.2 (D71)")?;
        let mut branches = vec![self.branch(opener)?];
        let mut else_body = None;
        while self.eat_word("else") {
            if self.eat_word("if") {
                branches.push(self.branch(opener)?);
            } else {
                self.expect(
                    &TokenKind::Colon,
                    "`:` after `else`",
                    "a block's opening line ends with `:` (D13, D71)",
                )?;
                else_body = Some(self.statements(opener)?);
                break;
            }
        }
        self.end_line("if", None, opener)?;
        Ok(StatementKind::If {
            branches,
            else_body,
        })
    }

    fn branch(&mut self, opener: Place) -> Result<Branch, Problem> {
        let condition = self.expression()?;
        self.expect(
            &TokenKind::Colon,
            "`:` after the condition",
            "a block's opening line ends with `:` (D13)",
        )?;
        let body = self.statements(opener)?;
        Ok(Branch { condition, body })
    }

    fn for_loop(&mut self) -> Result<StatementKind, Problem> {
        let opener = self.place();
        self.expect_word("for", "SSS: Radium 6.1 (D74)")?;
        let loop_name = if self.at_any_word() {
            Some(self.take_word("a loop name", "a loop may be named (D77)")?)
        } else {
            None
        };
        let variable = self.take_value_name("the loop's value, starting with `$`")?;
        self.expect_word("in", "every loop is `for $x in …:` (D74)")?;
        if matches!(self.kind(), Some(TokenKind::ValueName(_))) {
            return Err(Problem::not_built(
                self.place(),
                "going through a collection (D74)",
            ));
        }
        let range = self.loop_range()?;
        self.expect(
            &TokenKind::Colon,
            "`:` after the range",
            "a block's opening line ends with `:` (D13)",
        )?;
        let body = self.statements(opener)?;
        self.end_line("for", loop_name.as_deref(), opener)?;
        Ok(StatementKind::For {
            loop_name,
            variable,
            range,
            body,
        })
    }

    fn loop_range(&mut self) -> Result<LoopRange, Problem> {
        let start = self.place();
        self.expect_word("range", "a loop goes through a `range[…]` (D74)")?;
        self.expect(
            &TokenKind::OpenBracket,
            "`[` after `range`",
            "`[ ]` describes, so a range's labels go inside it (D50, D74)",
        )?;
        let mut from = None;
        let mut end = None;
        let mut step_by = None;
        let mut last_order = 0_u8;
        loop {
            let label_place = self.place();
            let label = self.take_word(
                "a label: `from:`, `to:`, `before:` or `step_by:`",
                "a range's labels are `from:`, `to:`, `before:` and `step_by:` (D73)",
            )?;
            self.expect(
                &TokenKind::Colon,
                "`:` after the label",
                "a label is a word and `:` (D54)",
            )?;
            let value = self.expression()?;
            let order = match label.as_str() {
                "from" => {
                    from = Some(value);
                    1
                }
                "to" | "before" => {
                    if end.is_some() {
                        return Err(Problem::refused(
                            label_place,
                            "a second end for this range",
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
                        label_place,
                        format!("`{other}:` isn't a range label"),
                        "a range's labels are `from:`, `to:`, `before:` and `step_by:` (D73)",
                    ));
                }
            };
            if order <= last_order {
                return Err(Problem::refused(
                    label_place,
                    format!("`{label}:` is out of order"),
                    "range labels go `from:`, then `to:` or `before:`, then `step_by:` (D54, D113)",
                ));
            }
            last_order = order;
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(
            &TokenKind::CloseBracket,
            "`]` closing the range",
            "a range is `range[…]` (D74)",
        )?;
        let place = self.span_from(start);
        let from = from.ok_or_else(|| {
            Problem::refused(
                place,
                "this range has no `from:`",
                "a range starts with `from:`, so a loop's start is always written (D113)",
            )
        })?;
        let end = end.ok_or_else(|| {
            Problem::refused(
                place,
                "this range has no end",
                "a range needs `to:` or `before:` (D73)",
            )
        })?;
        Ok(LoopRange { from, end, step_by })
    }

    fn while_loop(&mut self) -> Result<StatementKind, Problem> {
        let opener = self.place();
        self.expect_word("while", "SSS: Radium 6.2 (D75)")?;
        let condition = self.expression()?;
        self.expect(
            &TokenKind::Colon,
            "`:` after the condition",
            "a block's opening line ends with `:` (D13)",
        )?;
        let decreases = if self.eat_word("decreases") {
            let measure = self.expression()?;
            self.expect_semicolon()?;
            Some(measure)
        } else {
            None
        };
        let body = self.statements(opener)?;
        self.end_line("while", None, opener)?;
        Ok(StatementKind::While {
            condition,
            decreases,
            body,
        })
    }

    fn forever_loop(&mut self) -> Result<StatementKind, Problem> {
        let opener = self.place();
        self.expect_word("forever", "SSS: Radium 6.3 (D76)")?;
        let loop_name = if self.at_any_word() {
            Some(self.take_word("a loop name", "a loop may be named (D77)")?)
        } else {
            None
        };
        self.expect(
            &TokenKind::Colon,
            "`:` after `forever`",
            "a block's opening line ends with `:` (D13, D76)",
        )?;
        let body = self.statements(opener)?;
        self.end_line("forever", loop_name.as_deref(), opener)?;
        Ok(StatementKind::Forever { loop_name, body })
    }

    fn return_statement(&mut self) -> Result<StatementKind, Problem> {
        let start = self.place();
        self.expect_word("return", "SSS: Radium 7.4 (D95)")?;
        if self.at(&TokenKind::Semicolon) {
            return Err(Problem::refused(
                self.span_from(start),
                "a bare `return;`",
                "every `return` says what it gives back; a `-> void` function returns with `return void;` (D100)",
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
        self.expect_word("break", "SSS: Radium 6.4 (D77)")?;
        let loop_name = if self.at_any_word() {
            Some(self.take_word("a loop name", "`break name;` names a loop (D77)")?)
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
        let start = self.place();
        let target = if self.eat_word("std") {
            self.expect(
                &TokenKind::DoubleColon,
                "`::` after `std`",
                "`std::` marks the standard compound (D23)",
            )?;
            let name =
                self.take_word("a name from `std`", "`std::` is followed by a name (D23)")?;
            if name != "print" {
                return Err(Problem::not_built(
                    self.span_from(start),
                    format!("`std::{name}` (the oracle's `std` has only `print` so far)"),
                ));
            }
            if self.at(&TokenKind::Dot) {
                return Err(Problem::not_built(
                    self.place(),
                    "narrowings of `std::print` (D57)",
                ));
            }
            CallTarget::Print
        } else if self.at_word("my") && self.kind_after(1) == Some(&TokenKind::Dot) {
            self.advance();
            self.advance();
            CallTarget::Own {
                name: self.take_word(
                    "a function's name after `my.`",
                    "`my.` marks the writer's own function (D43)",
                )?,
                marked: true,
            }
        } else {
            CallTarget::Own {
                name: self.take_word("a function's name", "a call names a function (D50)")?,
                marked: false,
            }
        };
        self.expect(
            &TokenKind::OpenParen,
            "`(`",
            "a call runs something, so it uses `( )` (D50)",
        )?;
        let mut arguments = Vec::new();
        if !self.eat(&TokenKind::CloseParen) {
            loop {
                arguments.push(self.argument()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(
                &TokenKind::CloseParen,
                "`)` closing the call",
                "a call's arguments go inside `( )` (D50)",
            )?;
        }
        Ok(Call {
            place: self.span_from(start),
            target,
            arguments,
        })
    }

    fn argument(&mut self) -> Result<Argument, Problem> {
        let start = self.place();
        let label = if self.at_any_word() && self.kind_after(1) == Some(&TokenKind::Colon) {
            let label = self.take_word("a label", "a label is a word and `:` (D54)")?;
            self.advance();
            Some(label)
        } else {
            None
        };
        let handoff = self.handoff()?;
        let value = self.expression()?;
        if handoff.is_some() && !matches!(value.kind, ExpressionKind::Value(_)) {
            return Err(Problem::not_decided(
                value.place,
                "handing over something other than a whole value's `$name`, such as a field",
            ));
        }
        Ok(Argument {
            place: self.span_from(start),
            label,
            handoff,
            value,
        })
    }

    /// Reads `Give`, `Copy`, `Lend` or `Lend.mut` before an argument (D80, D84).
    fn handoff(&mut self) -> Result<Option<Handoff>, Problem> {
        let Some(TokenKind::Word(word)) = self.kind() else {
            return Ok(None);
        };
        let place = self.place();
        let followed_by_value = matches!(
            self.kind_after(1),
            Some(TokenKind::ValueName(_) | TokenKind::Dot)
        );
        let handoff = match word.as_str() {
            "Give" => Handoff::Give,
            "Copy" => Handoff::Copy,
            "Lend" => {
                if self.kind_after(1) == Some(&TokenKind::Dot) {
                    self.advance();
                    self.advance();
                    let narrowing = self
                        .take_word("`mut` after `Lend.`", "`Lend` narrows to `Lend.mut` (D80)")?;
                    if narrowing != "mut" {
                        return Err(Problem::refused(
                            self.span_from(place),
                            format!("`Lend.{narrowing}` isn't an ownership word"),
                            "the ownership words are `Give`, `Copy`, `Lend` and `Lend.mut` (D80, D84)",
                        ));
                    }
                    return Ok(Some(Handoff::LendMut));
                }
                Handoff::Lend
            }
            "give" | "copy" | "lend" if followed_by_value => {
                return Err(Problem::refused(
                    place,
                    format!("`{word}` in lowercase"),
                    "ownership words start with a capital, so every hand-off stands out (D84)",
                )
                .fix(format!("`{}`", capitalised(word))));
            }
            _ => return Ok(None),
        };
        self.advance();
        Ok(Some(handoff))
    }
}

/// `give` → `Give` (D84).
fn capitalised(word: &str) -> String {
    let mut characters = word.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

impl Parser {
    fn function(&mut self) -> Result<Function, Problem> {
        let start = self.place();
        self.expect_word("fn", "a function starts with `fn` (D51)")?;
        let narrowing = if self.eat(&TokenKind::Dot) {
            Some(self.take_word(
                "`pure` or `main` after `fn.`",
                "`fn` narrows to `fn.pure` (D51) or `fn.main` (D101)",
            )?)
        } else {
            None
        };
        let (pure, is_main) = match narrowing.as_deref() {
            None => (false, false),
            Some("pure") => (true, false),
            Some("main") => (false, true),
            Some(other) => {
                return Err(Problem::refused(
                    self.span_from(start),
                    format!("`fn.{other}` isn't a kind of function"),
                    "functions are `fn`, `fn.pure` (D51) and `fn.main` (D101)",
                ));
            }
        };
        if self.at(&TokenKind::Dot) {
            return Err(Problem::refused(
                self.place(),
                "a second narrowing of `fn`",
                "a function takes one narrowing: `fn.pure` or `fn.main` (D51, D101)",
            ));
        }
        let name = if is_main {
            "fn.main".to_owned()
        } else {
            self.take_word(
                "the function's name",
                "`fn` is followed by the function's name (D51)",
            )?
        };
        let place = self.span_from(start);
        self.expect(
            &TokenKind::OpenBracket,
            "`[` before the parameters",
            "a definition describes, so its parameters go in `[ ]` (D50)",
        )?;
        let mut parameters = Vec::new();
        if !self.eat(&TokenKind::CloseBracket) {
            loop {
                parameters.push(self.parameter()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(
                &TokenKind::CloseBracket,
                "`]` after the parameters",
                "a definition's parameters go in `[ ]` (D50)",
            )?;
        }
        if !self.eat(&TokenKind::Arrow) {
            return Err(Problem::refused(
                self.previous(),
                format!("`{name}` doesn't say what it gives back"),
                "every function states its result after `->`; one that gives nothing back says `-> void` (D98, D99)",
            ));
        }
        let result = if self.eat_word("void") {
            None
        } else {
            Some(self.type_spec()?)
        };
        self.expect(
            &TokenKind::Colon,
            "`:` ending the function's header",
            "a block's opening line ends with `:` (D13)",
        )?;
        let top = self.body_top()?;
        let body = self.statements(place)?;
        if is_main {
            self.end_main_line(place)?;
        } else {
            self.end_line("fn", Some(&name), place)?;
        }
        Ok(Function {
            name,
            place,
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
        let start = self.place();
        let changeability = if self.eat_word("var") {
            if self.eat(&TokenKind::Dot) {
                match self
                    .take_word(
                        "`mut` after `var.`",
                        "a parameter may be `var` or `var.mut` (D31)",
                    )?
                    .as_str()
                {
                    "mut" => Changeability::MustChange,
                    "immut" => {
                        return Err(Problem::refused(
                            self.span_from(start),
                            "`var.immut` on a parameter",
                            "a parameter is `var.immut` already, and a default is never spelled out (D31, D39)",
                        )
                        .fix("write the parameter bare"));
                    }
                    other => {
                        return Err(Problem::refused(
                            self.span_from(start),
                            format!("`var.{other}` isn't a kind of parameter"),
                            "a parameter is bare, `var` or `var.mut` (D31)",
                        ));
                    }
                }
            } else {
                Changeability::Free
            }
        } else {
            Changeability::Fixed
        };
        let name = self.take_value_name("a parameter's name, starting with `$`")?;
        self.expect(
            &TokenKind::Colon,
            "`:` and the parameter's type",
            "a parameter is `$name: type` (D50)",
        )?;
        let holding = self.holding()?;
        let type_spec = self.type_spec()?;
        Ok(Parameter {
            name,
            place: self.span_from(start),
            changeability,
            holding,
            type_spec,
        })
    }

    /// Reads how a parameter holds its value: bare (lent), `Lent.mut` or `Given` (D91, D92).
    fn holding(&mut self) -> Result<Holding, Problem> {
        let Some(TokenKind::Word(word)) = self.kind() else {
            return Ok(Holding::Lent);
        };
        let place = self.place();
        let word = word.clone();
        match word.as_str() {
            "Lent" if self.kind_after(1) == Some(&TokenKind::Dot) => {
                self.advance();
                self.advance();
                let narrowing =
                    self.take_word("`mut` after `Lent.`", "`Lent` narrows to `Lent.mut` (D91)")?;
                if narrowing == "mut" {
                    Ok(Holding::LentMut)
                } else {
                    Err(Problem::refused(
                        self.span_from(place),
                        format!("`Lent.{narrowing}` isn't a way to hold a parameter"),
                        "a parameter is bare (lent), `Lent.mut` or `Given` (D91, D92)",
                    ))
                }
            }
            "Lent" => Err(Problem::refused(
                place,
                "`Lent` written out",
                "a bare parameter is lent, and a default is never spelled out (D39, D92)",
            )
            .fix("write the parameter bare")),
            "Given" => {
                self.advance();
                Ok(Holding::Given)
            }
            "Lend" | "Give" | "Copy" => Err(Problem::refused(
                place,
                format!("`{word}` in a parameter"),
                "parameters say `Lent.mut` or `Given`; `Lend`, `Give` and `Copy` are for calls (D91)",
            )),
            "lent" | "given" | "lend" | "give" | "copy" => Err(Problem::refused(
                place,
                format!("`{word}` in lowercase"),
                "ownership words start with a capital (D84)",
            )
            .fix(format!("`{}`", capitalised(&word)))),
            _ => Ok(Holding::Lent),
        }
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
            let place = self.place();
            let next_stage = if self.at_word("labels") {
                1
            } else if self.at_word("defaults") {
                2
            } else if self.at_word("requires") || self.at_word("ensures") {
                3
            } else if self.at_word("fails") {
                return Err(Problem::not_decided(
                    place,
                    "the `fails … exactly when` wording (D56)",
                ));
            } else if self.at_word("decreases") {
                return Err(Problem::not_built(
                    place,
                    "`decreases` on a function (D4, D7)",
                ));
            } else {
                break;
            };
            if next_stage < stage || (next_stage == stage && next_stage < 3) {
                return Err(Problem::refused(
                    place,
                    "this line is out of order at the top of the body",
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
        let start = self.place();
        self.expect_word("labels", "SSS: Radium 7.3 (D54)")?;
        let any_order = if self.eat(&TokenKind::Dot) {
            match self
                .take_word(
                    "`any_order` after `labels.`",
                    "`labels` narrows to `labels.any_order` (D54)",
                )?
                .as_str()
            {
                "any_order" => true,
                "in_order" => {
                    return Err(Problem::refused(
                        self.span_from(start),
                        "`labels.in_order` spells out the default",
                        "bare `labels` keeps the declared order, and a default is never spelled out (D39, D54)",
                    )
                    .fix("`labels`"));
                }
                other => {
                    return Err(Problem::refused(
                        self.span_from(start),
                        format!("`labels.{other}` isn't a kind of labels line"),
                        "a labels line is `labels` or `labels.any_order` (D54)",
                    ));
                }
            }
        } else {
            false
        };
        self.expect(
            &TokenKind::OpenBracket,
            "`[` after `labels`",
            "the labels go in `[ ]` (D50, D54)",
        )?;
        let mut entries = Vec::new();
        loop {
            let parameter = self.take_value_name("a parameter's `$name`")?;
            self.expect_word("as", "a label entry is `$parameter as label` (D54)")?;
            let label = self.take_word(
                "the label callers write",
                "a label entry is `$parameter as label` (D54)",
            )?;
            entries.push(LabelEntry { parameter, label });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(
            &TokenKind::CloseBracket,
            "`]` closing the labels",
            "the labels go in `[ ]` (D54)",
        )?;
        self.expect_semicolon()?;
        Ok(Labels {
            place: self.span_from(start),
            any_order,
            entries,
        })
    }

    fn defaults_line(&mut self) -> Result<Vec<ParameterDefault>, Problem> {
        self.expect_word("defaults", "SSS: Radium 7.3 (D54)")?;
        if self.at(&TokenKind::Dot) {
            return Err(Problem::not_built(
                self.place(),
                "`defaults.fixed` (D57), which is for function collections",
            ));
        }
        self.expect(
            &TokenKind::OpenBracket,
            "`[` after `defaults`",
            "the defaults go in `[ ]` (D50, D54)",
        )?;
        let mut defaults = Vec::new();
        loop {
            let start = self.place();
            let parameter = self.take_value_name("a parameter's `$name`")?;
            self.expect(
                &TokenKind::Equals,
                "`=` and the default value",
                "a default is `$parameter = value` (D54)",
            )?;
            let value = self.expression()?;
            defaults.push(ParameterDefault {
                place: self.span_from(start),
                parameter,
                value,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(
            &TokenKind::CloseBracket,
            "`]` closing the defaults",
            "the defaults go in `[ ]` (D54)",
        )?;
        self.expect_semicolon()?;
        Ok(defaults)
    }

    fn type_spec(&mut self) -> Result<TypeSpec, Problem> {
        let start = self.place();
        let name = self.take_word("a type", "a type follows `:` (D30)")?;
        match name.as_str() {
            "bool" => Ok(TypeSpec::Bool),
            "int" => self.integer_type(start),
            "infer" => Err(Problem::refused(
                start,
                "`infer` on a parameter or result",
                "`infer` is for local values only; parameters and results write their types (D14, D108)",
            )),
            "string" => {
                if self.eat(&TokenKind::Dot) && self.eat_word("utf8") {
                    return Err(Problem::refused(
                        self.span_from(start),
                        "`string.utf8` spells out the default",
                        "a bare `string` is UTF-8, and a default is never spelled out (D39, D120)",
                    )
                    .fix("`string`"));
                }
                Err(Problem::not_decided(
                    start,
                    "text types, beyond \"a bare `string` is UTF-8\" (D120)",
                ))
            }
            "void" => Err(Problem::refused(
                start,
                "`void` as a value's type",
                "`void` is only a function's result, `-> void` (D99)",
            )),
            "str" => Err(Problem::refused(
                start,
                "`str` is a short form",
                "short forms are written in full unless they save 4 or more letters (D94)",
            )
            .fix("`string`")),
            "array" => self.array_type(),
            other if is_radium_word(other) => Err(Problem::refused(
                start,
                format!("`{other}` isn't a type"),
                "a type is `int[…]`, `bool`, `array[…]` or a declared `struct` or `union.tagged` (D58, D59, D70)",
            )),
            other => Ok(TypeSpec::Named(other.to_owned())),
        }
    }

    /// `array[element, length: N]` (D70).
    fn array_type(&mut self) -> Result<TypeSpec, Problem> {
        self.expect(
            &TokenKind::OpenBracket,
            "`[` after `array`",
            "an array's element and length go in `[ ]` (D50, D70)",
        )?;
        let element = self.type_spec()?;
        self.expect(
            &TokenKind::Comma,
            "`,` and then `length:`",
            "an array type is `array[element, length: N]` (D70)",
        )?;
        self.expect_word(
            "length",
            "an array type is `array[element, length: N]` (D70)",
        )?;
        self.expect(
            &TokenKind::Colon,
            "`:` after `length`",
            "a label is a word and `:` (D54)",
        )?;
        let length_place = self.place();
        let Some(TokenKind::Integer(length)) = self.kind().cloned() else {
            return Err(Problem::not_built(
                length_place,
                "an array length written with a named constant",
            ));
        };
        self.advance();
        self.expect(
            &TokenKind::CloseBracket,
            "`]` closing the array type",
            "an array type is `array[element, length: N]` (D70)",
        )?;
        if length.sign() != Sign::Plus {
            return Err(Problem::refused(
                length_place,
                "an array of no items",
                "an array holds a fixed number of values, at least one (D70)",
            ));
        }
        Ok(TypeSpec::Array {
            element: Box::new(element),
            length,
        })
    }

    /// `int[…]`, `int.wide[…]`, or an exact `int.signed.N_bit[…]` / `int.unsigned.N_bit[…]` (D14, D26, D27, D105,
    /// D107).
    fn integer_type(&mut self, start: Place) -> Result<TypeSpec, Problem> {
        // `None` is bare `int`, `Some(None)` is `int.wide`, `Some(Some(…))` an exact width.
        let width = if !self.eat(&TokenKind::Dot) {
            None
        } else if self.eat_word("wide") {
            Some(None)
        } else {
            Some(Some(self.integer_width(start)?))
        };
        if !self.eat(&TokenKind::OpenBracket) {
            return Err(Problem::not_decided(
                self.span_from(start),
                "an integer type without bounds (D26)",
            ));
        }
        if self.at_word("max") {
            return Err(Problem::not_decided(
                self.place(),
                "whether `max:` may come before `min:`",
            ));
        }
        self.expect_word("min", "an integer's bounds are `min:` then `max:` (D14)")?;
        self.expect(
            &TokenKind::Colon,
            "`:` after `min`",
            "a label is a word and `:` (D54)",
        )?;
        let min = self.bound()?;
        self.expect(
            &TokenKind::Comma,
            "`,` and then `max:`",
            "an integer's bounds are `min:` then `max:` (D14)",
        )?;
        self.expect_word("max", "an integer's bounds are `min:` then `max:` (D14)")?;
        self.expect(
            &TokenKind::Colon,
            "`:` after `max`",
            "a label is a word and `:` (D54)",
        )?;
        let max = self.bound()?;
        self.expect(
            &TokenKind::CloseBracket,
            "`]` closing the bounds",
            "an integer's bounds go in `[ ]` (D14, D50)",
        )?;
        let place = self.span_from(start);
        let bounds = Bounds { min, max };
        match width {
            Some(Some((signed, bits))) => check_width(place, &bounds, signed, bits)?,
            // trace D107: `int.wide` is only for bounds that need more than 64 bits.
            Some(None) if fits_64_bits(&bounds) => {
                return Err(Problem::refused(
                    place,
                    "these bounds fit in 64 bits, so `int.wide` isn't needed",
                    "`int.wide` is only for bounds that need more than 64 bits (D107)",
                )
                .fix("`int[…]`"));
            }
            // trace D105: a bare `int` holds at most 64 bits; wider ones say so.
            None if !fits_64_bits(&bounds) => {
                return Err(Problem::refused(
                    place,
                    "these bounds need more than 64 bits",
                    "a bare `int` holds up to 64 bits; wider integers are `int.wide[…]` or an exact width (D105, D107)",
                ));
            }
            Some(None) | None => {}
        }
        Ok(TypeSpec::Integer(bounds))
    }

    /// Reads `signed.N_bit` or `unsigned.N_bit` after `int.`.
    fn integer_width(&mut self, start: Place) -> Result<(bool, u32), Problem> {
        let signed = match self
            .take_word(
                "`wide`, `signed` or `unsigned` after `int.`",
                "`int` narrows to `int.wide`, `int.signed` or `int.unsigned` (D26, D107)",
            )?
            .as_str()
        {
            "signed" => true,
            "unsigned" => false,
            other => {
                return Err(Problem::refused(
                    self.span_from(start),
                    format!("`int.{other}` isn't a narrowing of `int`"),
                    "`int` narrows to `int.wide`, `int.signed` or `int.unsigned` (D26, D107)",
                ));
            }
        };
        if !self.eat(&TokenKind::Dot) {
            return Err(Problem::not_decided(
                self.span_from(start),
                "a signedness without a width, such as `int.signed[…]`",
            ));
        }
        let width_place = self.place();
        let width = self.take_word(
            "a width such as `32_bit`",
            "a width is written like `32_bit` (D26)",
        )?;
        let bits = width
            .strip_suffix("_bit")
            .and_then(|digits| digits.parse::<u32>().ok())
            .filter(|bits| *bits > 0)
            .ok_or_else(|| {
                Problem::refused(
                    width_place,
                    format!("`{width}` isn't a width"),
                    "a width is written like `32_bit` (D26)",
                )
            })?;
        Ok((signed, bits))
    }

    fn bound(&mut self) -> Result<BigInt, Problem> {
        let negative = self.eat(&TokenKind::Minus);
        let Some(TokenKind::Integer(number)) = self.kind() else {
            return Err(Problem::not_built(
                self.place(),
                "bounds written with named constants",
            ));
        };
        let number = number.clone();
        self.advance();
        Ok(if negative { -number } else { number })
    }
}

// Operator strength (D111): `-` before a value; `x`, `div`, `mod`; `+`, `-`; comparisons; `not`; then `and`/`or`,
// which don't mix without brackets.
impl Parser {
    fn expression(&mut self) -> Result<Expression, Problem> {
        self.or_expression()
    }

    fn or_expression(&mut self) -> Result<Expression, Problem> {
        let (mut left, mut has_and) = self.and_chain()?;
        let mut has_or = false;
        while self.at_word("or") {
            self.advance();
            self.refuse_logic_narrowing()?;
            let (right, right_has_and) = self.and_chain()?;
            has_and |= right_has_and;
            has_or = true;
            left = binary(Operator::Or, left, right);
        }
        // trace D111: `and` and `or` don't mix without brackets.
        if has_and && has_or {
            return Err(Problem::refused(
                left.place,
                "`and` and `or` mixed without brackets",
                "`and` and `or` don't mix without brackets, so their grouping is always written (D111)",
            ));
        }
        Ok(left)
    }

    /// Reads `a and b and …`, saying whether it held any `and` outside brackets.
    fn and_chain(&mut self) -> Result<(Expression, bool), Problem> {
        let mut left = self.not_expression()?;
        let mut has_and = false;
        while self.at_word("and") {
            self.advance();
            self.refuse_logic_narrowing()?;
            let right = self.not_expression()?;
            has_and = true;
            left = binary(Operator::And, left, right);
        }
        Ok((left, has_and))
    }

    fn refuse_logic_narrowing(&self) -> Result<(), Problem> {
        if self.at(&TokenKind::Dot) {
            Err(Problem::not_decided(
                self.place(),
                "the spelling of the `and`/`or` that evaluates both sides (D40)",
            ))
        } else {
            Ok(())
        }
    }

    fn not_expression(&mut self) -> Result<Expression, Problem> {
        if self.at_word("not") {
            let start = self.place();
            self.advance();
            let operand = self.not_expression()?;
            return Ok(Expression {
                place: start.to(operand.place),
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
        self.advance();
        let right = self.additive()?;
        if self.comparison_operator().is_some() {
            return Err(Problem::not_decided(
                self.place(),
                "chained comparisons such as `1 < $x < 5`",
            ));
        }
        Ok(binary(operator, left, right))
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
            self.advance();
            let right = self.multiplicative()?;
            left = binary(operator, left, right);
        }
    }

    fn multiplicative(&mut self) -> Result<Expression, Problem> {
        let mut left = self.unary()?;
        loop {
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
            left = binary(operator, left, right);
        }
    }

    fn rounding(&mut self) -> Result<Rounding, Problem> {
        let start = self.previous();
        if !self.eat(&TokenKind::Dot) {
            return Ok(Rounding::Exact);
        }
        let word = self.take_word(
            "`floor`, `trunc` or `ceil` after `div.`",
            "`div` narrows to `div.floor`, `div.trunc` or `div.ceil` (D37)",
        )?;
        let place = self.span_from(start);
        match word.as_str() {
            "floor" => Ok(Rounding::Floor),
            "trunc" => Ok(Rounding::Trunc),
            "ceil" => Ok(Rounding::Ceil),
            "exact" => Err(Problem::refused(
                place,
                "`div.exact` spells out the default",
                "bare `div` is exact, and a default the language fixes is never spelled out (D37, D39)",
            )
            .fix("`div`")),
            other => Err(Problem::refused(
                place,
                format!("`div.{other}` isn't a division"),
                "division is `div`, `div.floor`, `div.trunc` or `div.ceil` (D37)",
            )),
        }
    }

    fn remainder_rule(&mut self) -> Result<RemainderRule, Problem> {
        let start = self.previous();
        if !self.eat(&TokenKind::Dot) {
            return Ok(RemainderRule::NonNegative);
        }
        let word = self.take_word(
            "`floor`, `trunc` or `euclid` after `mod.`",
            "`mod` narrows to `mod.floor`, `mod.trunc` or `mod.euclid` (D38)",
        )?;
        let place = self.span_from(start);
        match word.as_str() {
            "floor" => Ok(RemainderRule::Floor),
            "trunc" => Ok(RemainderRule::Trunc),
            "euclid" => Ok(RemainderRule::Euclid),
            other => Err(Problem::refused(
                place,
                format!("`mod.{other}` isn't a remainder"),
                "the remainder is `mod`, `mod.floor`, `mod.trunc` or `mod.euclid` (D38)",
            )),
        }
    }

    fn unary(&mut self) -> Result<Expression, Problem> {
        if self.at(&TokenKind::Minus) {
            let start = self.place();
            self.advance();
            let operand = self.unary()?;
            return Ok(Expression {
                place: start.to(operand.place),
                kind: ExpressionKind::Negate(Box::new(operand)),
            });
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expression, Problem> {
        let start = self.place();
        let Some(kind) = self.kind().cloned() else {
            return Err(Problem::refused(
                start,
                "the program ends in the middle of a value",
                "a value is complete before its statement ends (D29)",
            ));
        };
        let expression_kind = match kind {
            TokenKind::Integer(number) => ExpressionKind::Integer(number),
            TokenKind::Text(text) => ExpressionKind::Text(text),
            TokenKind::ValueName(name) => {
                self.advance();
                return self.field_reads(Expression {
                    place: start,
                    kind: ExpressionKind::Value(name),
                });
            }
            TokenKind::OpenParen => {
                // trace D50: maths grouping is the one place `( )` runs nothing.
                self.advance();
                let inner = self.expression()?;
                self.expect(
                    &TokenKind::CloseParen,
                    "`)` closing the group",
                    "a group opened with `(` closes with `)` (D50)",
                )?;
                return Ok(Expression {
                    place: self.span_from(start),
                    kind: inner.kind,
                });
            }
            TokenKind::Word(word) => return self.word_value(start, &word),
            other => {
                return Err(Problem::refused(
                    start,
                    format!("{} can't start a value here", describe(&other)),
                    "a value starts with a number, text, a `$name`, a word such as `true`, a call, `-`, `not` or `(` (SSS: Radium 4)",
                ));
            }
        };
        self.advance();
        Ok(Expression {
            place: start,
            kind: expression_kind,
        })
    }

    fn word_value(&mut self, start: Place, word: &str) -> Result<Expression, Problem> {
        let kind = match word {
            "true" => ExpressionKind::Bool(true),
            "false" => ExpressionKind::Bool(false),
            "newline" => ExpressionKind::Newline,
            "newln" => {
                return Err(Problem::refused(
                    start,
                    "`newln` is a short form",
                    "short forms are written in full unless they save 4 or more letters (D94)",
                )
                .fix("`newline`"));
            }
            "old" => {
                self.advance();
                self.expect(
                    &TokenKind::OpenBracket,
                    "`[` after `old`",
                    "`old[$parameter]` describes a starting value, so it uses `[ ]` (D50, D53)",
                )?;
                let name = self.take_value_name("a parameter's `$name`")?;
                self.expect(
                    &TokenKind::CloseBracket,
                    "`]` closing `old[…]`",
                    "`old[$parameter]` (D53)",
                )?;
                return Ok(Expression {
                    place: self.span_from(start),
                    kind: ExpressionKind::Old(name),
                });
            }
            "result" => ExpressionKind::Result,
            "std" => {
                return Err(Problem::not_built(start, "`std` inside a value"));
            }
            "my" if self.kind_after(1) == Some(&TokenKind::Dot) => {
                let call = self.call()?;
                return Ok(Expression {
                    place: call.place,
                    kind: ExpressionKind::Call(call),
                });
            }
            _ if self.kind_after(1) == Some(&TokenKind::OpenParen) => {
                let call = self.call()?;
                return Ok(Expression {
                    place: call.place,
                    kind: ExpressionKind::Call(call),
                });
            }
            _ if self.kind_after(1) == Some(&TokenKind::OpenBracket) => {
                self.advance();
                let fields = self.field_values()?;
                return Ok(Expression {
                    place: self.span_from(start),
                    kind: ExpressionKind::Build {
                        type_name: word.to_owned(),
                        case: None,
                        fields,
                    },
                });
            }
            _ if self.kind_after(1) == Some(&TokenKind::Dot)
                && matches!(self.kind_after(2), Some(TokenKind::Word(_))) =>
            {
                self.advance();
                self.advance();
                let case = self.take_word(
                    "a case's name",
                    "a case is written with its type: `status.ok` (D59)",
                )?;
                let fields = if self.at(&TokenKind::OpenBracket) {
                    self.field_values()?
                } else {
                    Vec::new()
                };
                return Ok(Expression {
                    place: self.span_from(start),
                    kind: ExpressionKind::Build {
                        type_name: word.to_owned(),
                        case: Some(case),
                        fields,
                    },
                });
            }
            _ => {
                return Err(Problem::refused(
                    start,
                    format!("`{word}` can't start a value here"),
                    "a value starts with a number, text, a `$name`, a word such as `true`, a call, `-`, `not` or `(` (SSS: Radium 4)",
                ));
            }
        };
        self.advance();
        Ok(Expression { place: start, kind })
    }
}

impl Parser {
    /// Reads `.field` after a value, as often as written: `$r.celsius` (D26, D58).
    fn field_reads(&mut self, mut base: Expression) -> Result<Expression, Problem> {
        loop {
            if self.at(&TokenKind::OpenBracket) {
                return Err(Problem::not_built(
                    self.place(),
                    "reading an `array` item (D64, D70)",
                ));
            }
            if !(self.at(&TokenKind::Dot) && matches!(self.kind_after(1), Some(TokenKind::Word(_))))
            {
                return Ok(base);
            }
            self.advance();
            let name = self.take_word("a field's name", "`.` reads a field (D26, D58)")?;
            base = Expression {
                place: self.span_from(base.place),
                kind: ExpressionKind::Field {
                    base: Box::new(base),
                    name,
                },
            };
        }
    }

    /// Reads `[field = value, …]` when building a value (D50, D58).
    fn field_values(&mut self) -> Result<Vec<FieldValue>, Problem> {
        self.expect(
            &TokenKind::OpenBracket,
            "`[`",
            "a value is built with its fields in `[ ]` (D50, D58)",
        )?;
        let mut fields = Vec::new();
        if self.eat(&TokenKind::CloseBracket) {
            return Ok(fields);
        }
        loop {
            let start = self.place();
            let name = self.take_word(
                "a field's name",
                "a field is set as `field = value` (D50, D58)",
            )?;
            self.expect(
                &TokenKind::Equals,
                "`=` and the field's value",
                "a field is set as `field = value` (D50, D58)",
            )?;
            let value = self.expression()?;
            fields.push(FieldValue {
                place: self.span_from(start),
                name,
                value,
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(
            &TokenKind::CloseBracket,
            "`]` closing the fields",
            "a value is built with its fields in `[ ]` (D50, D58)",
        )?;
        Ok(fields)
    }

    /// Reads `.must_Give` after `struct` or `union.tagged`, if it is there (D84).
    fn must_give(&mut self) -> Result<bool, Problem> {
        if !self.at(&TokenKind::Dot) {
            return Ok(false);
        }
        let place = self.place();
        self.advance();
        let word = self.take_word("`must_Give`", "a type narrows to `must_Give` (D84)")?;
        if word == "must_Give" {
            return Ok(true);
        }
        Err(Problem::refused(
            self.span_from(place),
            format!("`.{word}` isn't a narrowing of a type"),
            "a `struct` or `union.tagged` may be narrowed with `must_Give` (D84)",
        ))
    }

    /// Reads `end` and the exact words of the opener, such as `end union.tagged.must_Give status` (D13).
    fn end_words(&mut self, words: &[&str], name: &str, opener: Place) -> Result<(), Problem> {
        let start = self.place();
        let wanted = format!("`end {} {name}`", words.join("."));
        let mut matched = self.eat_word("end");
        for (index, word) in words.iter().enumerate() {
            if index > 0 {
                matched = matched && self.eat(&TokenKind::Dot);
            }
            matched = matched && self.eat_word(word);
        }
        matched = matched && self.eat_word(name);
        if !matched
            || self
                .current()
                .is_some_and(|token| token.place.line == start.line)
        {
            while self
                .current()
                .is_some_and(|token| token.place.line == start.line)
            {
                self.advance();
            }
            return Err(Problem::refused(
                self.span_from(start),
                "this `end` doesn't match its opener",
                "every `end` matches its opener exactly (D13)",
            )
            .fix(wanted)
            .because(opener, "the block opens here"));
        }
        Ok(())
    }

    /// Reads `name: type;` or `name.immut: type;` (D58).
    fn field(&mut self) -> Result<Field, Problem> {
        let start = self.place();
        let name = self.take_word("a field's name", "a field is `name: type;` (D58)")?;
        let locked = if self.eat(&TokenKind::Dot) {
            let word = self.take_word(
                "`immut` after the field's name",
                "a field may be locked with `.immut` (D58)",
            )?;
            if word != "immut" {
                return Err(Problem::refused(
                    self.span_from(start),
                    format!("`.{word}` on a field"),
                    "a field may only be locked with `.immut`; there is no `.mut` for fields (D58)",
                ));
            }
            true
        } else {
            false
        };
        self.expect(
            &TokenKind::Colon,
            "`:` and the field's type",
            "a field is `name: type;` (D58)",
        )?;
        let type_spec = self.type_spec()?;
        Ok(Field {
            name,
            place: self.span_from(start),
            locked,
            type_spec,
        })
    }

    /// `struct name:` … `end struct name` (D58).
    fn struct_definition(&mut self) -> Result<StructDefinition, Problem> {
        let start = self.place();
        self.expect_word("struct", "SSS: Radium (D58)")?;
        let must_give = self.must_give()?;
        let name = self.take_word(
            "the struct's name",
            "`struct` is followed by its name (D58)",
        )?;
        let place = self.span_from(start);
        self.expect(
            &TokenKind::Colon,
            "`:` ending the struct's header",
            "a block's opening line ends with `:` (D13)",
        )?;
        let mut fields = Vec::new();
        while !self.at_word("end") {
            if self.current().is_none() {
                return Err(Problem::refused(
                    self.place(),
                    "the program ends inside a struct",
                    "every block closes with `end` and its kind (D13)",
                )
                .because(place, "the struct opens here"));
            }
            fields.push(self.field()?);
            self.expect_semicolon()?;
        }
        let words: &[&str] = if must_give {
            &["struct", "must_Give"]
        } else {
            &["struct"]
        };
        self.end_words(words, &name, place)?;
        Ok(StructDefinition {
            name,
            place,
            must_give,
            fields,
        })
    }

    /// `union.tagged name:` … `end union.tagged name` (D59).
    fn union_definition(&mut self) -> Result<UnionDefinition, Problem> {
        let start = self.place();
        self.expect_word("union", "SSS: Radium (D59)")?;
        if !(self.at(&TokenKind::Dot)
            && self.kind_after(1) == Some(&TokenKind::Word("tagged".to_owned())))
        {
            return Err(Problem::refused(
                start,
                "a bare `union` outside a shield",
                "a bare `union` is C's untagged kind, allowed only inside shields; elsewhere it is `union.tagged` (D59)",
            ));
        }
        self.advance();
        self.advance();
        let must_give = self.must_give()?;
        let name = self.take_word(
            "the union's name",
            "`union.tagged` is followed by its name (D59)",
        )?;
        let place = self.span_from(start);
        self.expect(
            &TokenKind::Colon,
            "`:` ending the union's header",
            "a block's opening line ends with `:` (D13)",
        )?;
        let mut cases = Vec::new();
        while !self.at_word("end") {
            if self.current().is_none() {
                return Err(Problem::refused(
                    self.place(),
                    "the program ends inside a union",
                    "every block closes with `end` and its kind (D13)",
                )
                .because(place, "the union opens here"));
            }
            let case_start = self.place();
            let case_name = self.take_word(
                "a case's name",
                "a case is `name;` or `name[field: type, …];` (D59)",
            )?;
            let mut fields = Vec::new();
            if self.eat(&TokenKind::OpenBracket) {
                loop {
                    fields.push(self.field()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(
                    &TokenKind::CloseBracket,
                    "`]` closing the case's fields",
                    "a case's data goes in `[ ]` (D50, D59)",
                )?;
            }
            cases.push(Case {
                name: case_name,
                place: self.span_from(case_start),
                fields,
            });
            self.expect_semicolon()?;
        }
        let words: &[&str] = if must_give {
            &["union", "tagged", "must_Give"]
        } else {
            &["union", "tagged"]
        };
        self.end_words(words, &name, place)?;
        Ok(UnionDefinition {
            name,
            place,
            must_give,
            cases,
        })
    }

    /// `match $x:` with `case type.case[field = $name]:` branches and one `end match` (D72).
    fn match_statement(&mut self) -> Result<StatementKind, Problem> {
        let opener = self.place();
        self.expect_word("match", "SSS: Radium 5.3 (D72)")?;
        let subject = self.expression()?;
        self.expect(
            &TokenKind::Colon,
            "`:` after what is matched",
            "a block's opening line ends with `:` (D13)",
        )?;
        let mut cases = Vec::new();
        while self.at_word("case") {
            let start = self.place();
            self.advance();
            let type_name = self.take_word(
                "the union's name",
                "a case is written with its type: `status.ok` (D59, D72)",
            )?;
            self.expect(
                &TokenKind::Dot,
                "`.` and the case",
                "a case is written with its type: `status.ok` (D59, D72)",
            )?;
            let case_name = self.take_word(
                "the case's name",
                "a case is written with its type: `status.ok` (D59, D72)",
            )?;
            let mut bindings = Vec::new();
            if self.eat(&TokenKind::OpenBracket) {
                loop {
                    let binding_start = self.place();
                    let field = self.take_word(
                        "a field's name",
                        "a case pulls out its data as `field = $name` (D72)",
                    )?;
                    self.expect(
                        &TokenKind::Equals,
                        "`=` and a `$name`",
                        "a case pulls out its data as `field = $name` (D72)",
                    )?;
                    let name = self.take_value_name("a `$name` to hold the field")?;
                    bindings.push(CaseBinding {
                        place: self.span_from(binding_start),
                        field,
                        name,
                    });
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(
                    &TokenKind::CloseBracket,
                    "`]` closing the case's data",
                    "a case pulls out its data in `[ ]` (D72)",
                )?;
            }
            let place = self.span_from(start);
            self.expect(
                &TokenKind::Colon,
                "`:` after the case",
                "a block's opening line ends with `:` (D13)",
            )?;
            let body = self.statements(opener)?;
            cases.push(MatchCase {
                place,
                type_name,
                case_name,
                bindings,
                body,
            });
        }
        if cases.is_empty() {
            return Err(self.expected(
                "`case`",
                "a `match` has one `case` per case of its union (D72)",
            ));
        }
        self.end_line("match", None, opener)?;
        Ok(StatementKind::Match { subject, cases })
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
fn check_width(place: Place, bounds: &Bounds, signed: bool, bits: u32) -> Result<(), Problem> {
    let (lowest, highest) = if signed {
        let half = BigInt::from(1) << bits.saturating_sub(1);
        (-half.clone(), half - 1)
    } else {
        (BigInt::from(0), (BigInt::from(1) << bits) - 1)
    };
    if bounds.min < lowest || bounds.max > highest {
        let kind = if signed { "signed" } else { "unsigned" };
        return Err(Problem::refused(
            place,
            format!(
                "`int.{kind}.{bits}_bit` holds {lowest} to {highest}, and these bounds don't fit"
            ),
            "an exact width must hold its bounds (D14, D27)",
        ));
    }
    Ok(())
}

fn binary(operator: Operator, left: Expression, right: Expression) -> Expression {
    Expression {
        place: left.place.to(right.place),
        kind: ExpressionKind::Binary {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        },
    }
}
// end section oracle.parser
