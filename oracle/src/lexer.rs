// section oracle.lexer:
//! Turns source text into tokens. Comments are removed here (D86, D87), and a stray `#` is refused (D87).

use std::iter::Peekable;
use std::str::CharIndices;

use num_bigint::BigInt;
use unicode_segmentation::UnicodeSegmentation;

use crate::outcome::{Place, Problem};

/// One token and where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) place: Place,
}

/// The kinds of token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// A bare name or keyword: `fn`, `div`, `first_word`, `x`.
    Word(String),
    /// A value's name, without its `$` (D31).
    ValueName(String),
    Integer(BigInt),
    /// Text between `"` and `"`.
    Text(String),
    Colon,
    DoubleColon,
    Semicolon,
    Comma,
    Dot,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    Equals,
    Plus,
    Minus,
    Less,
    Greater,
    Arrow,
}

/// Splits a whole program into tokens.
pub(crate) fn tokenize(source: &str) -> Result<Vec<Token>, Problem> {
    let mut tokens = Vec::new();
    let mut comment_block_start: Option<Place> = None;
    for (index, text) in source.lines().enumerate() {
        let line = index.saturating_add(1);
        let trimmed = text.trim();
        // trace D86: a comment block runs from `comment:` to `end comment`; blank lines inside belong to it.
        if comment_block_start.is_some() {
            if trimmed == "end comment" {
                comment_block_start = None;
            }
        } else if trimmed == "comment:" {
            let start = text.len().saturating_sub(text.trim_start().len());
            comment_block_start = Some(place_of(
                text,
                line,
                start,
                start.saturating_add(trimmed.len()),
            ));
        } else {
            Scanner {
                text,
                line,
                characters: text.char_indices().peekable(),
            }
            .scan(&mut tokens)?;
        }
    }
    if let Some(start) = comment_block_start {
        return Err(Problem::refused(
            start,
            "this `comment:` block never reaches `end comment`",
            "a comment block runs from `comment:` to `end comment` (D86)",
        ));
    }
    Ok(tokens)
}

/// The place of the bytes `start..end` of one line (D117): byte and grapheme columns, and the width in graphemes.
pub(crate) fn place_of(text: &str, line: usize, start: usize, end: usize) -> Place {
    let before = text.get(..start).unwrap_or_default();
    let part = text.get(start..end).unwrap_or_default();
    Place {
        line,
        byte_column: start.saturating_add(1),
        column: before.graphemes(true).count().saturating_add(1),
        width: part.graphemes(true).count().max(1),
    }
}

/// Reads the tokens of one line.
struct Scanner<'text> {
    text: &'text str,
    line: usize,
    characters: Peekable<CharIndices<'text>>,
}

impl Scanner<'_> {
    /// Where the next character is, or the end of the line.
    fn offset(&mut self) -> usize {
        self.characters
            .peek()
            .map_or(self.text.len(), |(offset, _)| *offset)
    }

    fn place(&self, start: usize, end: usize) -> Place {
        place_of(self.text, self.line, start, end)
    }

    /// From `start` up to the next character.
    fn place_to_here(&mut self, start: usize) -> Place {
        let end = self.offset();
        self.place(start, end)
    }

    /// From `start` through the next character.
    fn place_through_next(&mut self, start: usize) -> Place {
        let end = self
            .characters
            .clone()
            .nth(1)
            .map_or(self.text.len(), |(offset, _)| offset);
        self.place(start, end)
    }

    fn next_if_eq(&mut self, wanted: char) -> bool {
        self.characters
            .next_if(|(_, character)| *character == wanted)
            .is_some()
    }

    fn peek_char(&mut self) -> Option<char> {
        self.characters.peek().map(|(_, character)| *character)
    }

    fn scan(mut self, tokens: &mut Vec<Token>) -> Result<(), Problem> {
        while let Some((start, character)) = self.characters.next() {
            let kind = match character {
                ' ' | '\t' | '\r' => continue,
                '/' => {
                    if self.next_if_eq('/') {
                        return self.check_comment(start);
                    }
                    return Err(Problem::refused(
                        self.place_to_here(start),
                        "`/` has no meaning",
                        "Radium divides with the word `div` (D34, D37)",
                    ));
                }
                '#' => {
                    return Err(Problem::refused(
                        self.place_to_here(start),
                        "`#` has no meaning yet",
                        "a comment is `//` and a space; `#` is kept free (D87)",
                    ));
                }
                '"' => self.read_text(start)?,
                '$' => self.read_value_name(start)?,
                ':' => {
                    if self.next_if_eq(':') {
                        TokenKind::DoubleColon
                    } else {
                        TokenKind::Colon
                    }
                }
                '-' => {
                    if self.next_if_eq('>') {
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    }
                }
                ';' => TokenKind::Semicolon,
                ',' => TokenKind::Comma,
                '.' => TokenKind::Dot,
                '(' => TokenKind::OpenParen,
                ')' => TokenKind::CloseParen,
                '[' => TokenKind::OpenBracket,
                ']' => TokenKind::CloseBracket,
                '=' => TokenKind::Equals,
                '+' => TokenKind::Plus,
                '<' => TokenKind::Less,
                '>' => TokenKind::Greater,
                digit if digit.is_ascii_digit() => self.read_integer(start, digit)?,
                letter if letter.is_ascii_alphabetic() || letter == '_' => {
                    TokenKind::Word(self.read_name(String::from(letter)))
                }
                other => {
                    return Err(Problem::refused(
                        self.place_to_here(start),
                        format!("`{other}` has no meaning in Radium"),
                        "outside text and comments, only Radium's own symbols and words appear (D34)",
                    ));
                }
            };
            let end = self.offset();
            tokens.push(Token {
                kind,
                place: self.place(start, end),
            });
        }
        Ok(())
    }

    /// Checks what follows `//` (D87).
    fn check_comment(&mut self, start: usize) -> Result<(), Problem> {
        let after = self.offset();
        let rest = self.text.get(after..).unwrap_or_default();
        if rest.trim().is_empty() {
            return Err(Problem::refused(
                self.place(start, after),
                "a lone `//`",
                "a comment is `//`, a space and text; paragraph breaks go in a `comment:` block (D86, D87)",
            ));
        }
        if !rest.starts_with(' ') {
            return Err(Problem::refused(
                self.place(start, after),
                "`//` with no space after it",
                "a comment is `//` followed by a space (D87)",
            )
            .fix("`// ` followed by the text"));
        }
        Ok(())
    }

    fn read_text(&mut self, start: usize) -> Result<TokenKind, Problem> {
        let mut text = String::new();
        while let Some((offset, character)) = self.characters.next() {
            match character {
                '"' => return Ok(TokenKind::Text(text)),
                '\\' => {
                    return Err(Problem::not_decided(
                        self.place_to_here(offset),
                        "escapes inside text",
                    ));
                }
                other => text.push(other),
            }
        }
        Err(Problem::not_decided(
            self.place(start, self.text.len()),
            "text that runs past the end of its line",
        ))
    }

    fn read_value_name(&mut self, start: usize) -> Result<TokenKind, Problem> {
        if self.peek_char() == Some('"') {
            return Err(Problem::not_decided(
                self.place_through_next(start),
                "what may go inside a quoted name `$\"…\"` (D32)",
            ));
        }
        let mut name = String::new();
        while let Some((_, character)) = self
            .characters
            .next_if(|(_, character)| character.is_ascii_alphanumeric() || *character == '_')
        {
            name.push(character);
        }
        if name.is_empty() {
            return Err(Problem::refused(
                self.place_to_here(start),
                "`$` with no name after it",
                "`$` starts a value's name (D31)",
            ));
        }
        Ok(TokenKind::ValueName(name))
    }

    fn read_integer(&mut self, start: usize, first: char) -> Result<TokenKind, Problem> {
        if first == '0' && self.peek_char() == Some('x') {
            return Err(Problem::not_built(
                self.place_through_next(start),
                "hex numbers (`0x…`, D34)",
            ));
        }
        let mut written = String::from(first);
        while let Some((_, character)) = self
            .characters
            .next_if(|(_, character)| character.is_ascii_digit() || *character == '_')
        {
            written.push(character);
        }
        if self
            .peek_char()
            .is_some_and(|character| character.is_ascii_alphabetic())
        {
            // trace D26: a narrowing such as `32_bit` starts with digits and joins a word with `_` (D35).
            if written.ends_with('_') {
                return Ok(TokenKind::Word(self.read_name(written)));
            }
            let word_end = {
                let mut end = self.offset();
                while let Some((offset, character)) = self.characters.next() {
                    if !character.is_ascii_alphanumeric() {
                        end = offset;
                        break;
                    }
                    end = self.offset();
                }
                end
            };
            return Err(Problem::refused(
                self.place(start, word_end),
                "a number runs straight into a word",
                "`x` needs spaces around it, so a number and a word never touch (D34)",
            ));
        }
        let digits: String = written
            .chars()
            .filter(|character| *character != '_')
            .collect();
        digits
            .parse::<BigInt>()
            .map(TokenKind::Integer)
            .map_err(|_| {
                Problem::refused(
                    self.place_to_here(start),
                    format!("`{written}` isn't a number"),
                    "a number is decimal digits, with `_` allowed between them (SSS: Radium 3.3)",
                )
            })
    }

    fn read_name(&mut self, start: String) -> String {
        let mut name = start;
        while let Some((_, character)) = self
            .characters
            .next_if(|(_, character)| character.is_ascii_alphanumeric() || *character == '_')
        {
            name.push(character);
        }
        name
    }
}

/// How error messages name a token.
pub(crate) fn describe(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Word(word) => format!("`{word}`"),
        TokenKind::ValueName(name) => format!("`${name}`"),
        TokenKind::Integer(number) => format!("the number {number}"),
        TokenKind::Text(_) => "text".to_owned(),
        TokenKind::Colon => "`:`".to_owned(),
        TokenKind::DoubleColon => "`::`".to_owned(),
        TokenKind::Semicolon => "`;`".to_owned(),
        TokenKind::Comma => "`,`".to_owned(),
        TokenKind::Dot => "`.`".to_owned(),
        TokenKind::OpenParen => "`(`".to_owned(),
        TokenKind::CloseParen => "`)`".to_owned(),
        TokenKind::OpenBracket => "`[`".to_owned(),
        TokenKind::CloseBracket => "`]`".to_owned(),
        TokenKind::Equals => "`=`".to_owned(),
        TokenKind::Plus => "`+`".to_owned(),
        TokenKind::Minus => "`-`".to_owned(),
        TokenKind::Less => "`<`".to_owned(),
        TokenKind::Greater => "`>`".to_owned(),
        TokenKind::Arrow => "`->`".to_owned(),
    }
}
// end section oracle.lexer
