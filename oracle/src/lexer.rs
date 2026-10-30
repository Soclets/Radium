// section oracle.lexer:
//! Turns source text into tokens. Comments are removed here (D86, D87), and a stray `#` is refused (D87).

use std::iter::Peekable;
use std::str::Chars;

use crate::outcome::Problem;

/// One token and the line it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) line: usize,
}

/// The kinds of token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// A bare name or keyword: `fn`, `div`, `first_word`, `x`.
    Word(String),
    /// A value's name, without its `$` (D31).
    ValueName(String),
    Integer(i128),
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
    let mut comment_block_start: Option<usize> = None;
    for (index, text) in source.lines().enumerate() {
        let line = index.saturating_add(1);
        let trimmed = text.trim();
        // trace D86: a comment block runs from `comment:` to `end comment`; blank lines inside belong to it.
        if comment_block_start.is_some() {
            if trimmed == "end comment" {
                comment_block_start = None;
            }
        } else if trimmed == "comment:" {
            comment_block_start = Some(line);
        } else {
            scan_line(text, line, &mut tokens)?;
        }
    }
    if let Some(start) = comment_block_start {
        return Err(Problem::refused(
            start,
            "this `comment:` block never reaches `end comment` (D86)",
        ));
    }
    Ok(tokens)
}

fn scan_line(text: &str, line: usize, tokens: &mut Vec<Token>) -> Result<(), Problem> {
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        let kind = match character {
            ' ' | '\t' | '\r' => continue,
            '/' => {
                if characters.next_if_eq(&'/').is_some() {
                    let rest: String = characters.collect();
                    return check_comment(&rest, line);
                }
                return Err(Problem::refused(
                    line,
                    "`/` has no meaning: Radium divides with `div` (D34, D37)",
                ));
            }
            '#' => {
                return Err(Problem::refused(
                    line,
                    "`#` has no meaning yet; a comment is `//` and a space (D87)",
                ));
            }
            '"' => read_text(&mut characters, line)?,
            '$' => read_value_name(&mut characters, line)?,
            ':' => {
                if characters.next_if_eq(&':').is_some() {
                    TokenKind::DoubleColon
                } else {
                    TokenKind::Colon
                }
            }
            '-' => {
                if characters.next_if_eq(&'>').is_some() {
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
            digit if digit.is_ascii_digit() => read_integer(digit, &mut characters, line)?,
            letter if letter.is_ascii_alphabetic() || letter == '_' => {
                TokenKind::Word(read_name(letter, &mut characters))
            }
            other => {
                return Err(Problem::refused(
                    line,
                    format!("`{other}` has no meaning in Radium"),
                ));
            }
        };
        tokens.push(Token { kind, line });
    }
    Ok(())
}

/// Checks what follows `//` on a line (D87).
fn check_comment(rest: &str, line: usize) -> Result<(), Problem> {
    if rest.trim().is_empty() {
        return Err(Problem::refused(
            line,
            "a lone `//` is refused; paragraph breaks go in a `comment:` block (D86, D87)",
        ));
    }
    if !rest.starts_with(' ') {
        return Err(Problem::refused(
            line,
            "`//` must be followed by a space (D87)",
        ));
    }
    Ok(())
}

fn read_text(characters: &mut Peekable<Chars<'_>>, line: usize) -> Result<TokenKind, Problem> {
    let mut text = String::new();
    for character in characters.by_ref() {
        match character {
            '"' => return Ok(TokenKind::Text(text)),
            '\\' => {
                return Err(Problem::not_decided(
                    line,
                    "escapes inside text aren't decided",
                ));
            }
            other => text.push(other),
        }
    }
    Err(Problem::not_decided(
        line,
        "text that runs past the end of its line isn't decided",
    ))
}

fn read_value_name(
    characters: &mut Peekable<Chars<'_>>,
    line: usize,
) -> Result<TokenKind, Problem> {
    if characters.peek() == Some(&'"') {
        return Err(Problem::not_decided(
            line,
            "what may go inside a quoted name `$\"…\"` is open (D32)",
        ));
    }
    let mut name = String::new();
    while let Some(character) =
        characters.next_if(|character| character.is_ascii_alphanumeric() || *character == '_')
    {
        name.push(character);
    }
    if name.is_empty() {
        return Err(Problem::refused(
            line,
            "`$` starts a value's name, so a name must follow it (D31)",
        ));
    }
    Ok(TokenKind::ValueName(name))
}

fn read_integer(
    first: char,
    characters: &mut Peekable<Chars<'_>>,
    line: usize,
) -> Result<TokenKind, Problem> {
    if first == '0' && characters.peek() == Some(&'x') {
        return Err(Problem::not_built(
            line,
            "hex numbers (`0x…`, D34) are in a later slice",
        ));
    }
    let mut digits = String::from(first);
    while let Some(character) =
        characters.next_if(|character| character.is_ascii_digit() || *character == '_')
    {
        if character != '_' {
            digits.push(character);
        }
    }
    if characters.peek().is_some_and(char::is_ascii_alphabetic) {
        return Err(Problem::refused(
            line,
            "a number runs straight into a word; `x` needs spaces around it (D34)",
        ));
    }
    digits
        .parse::<i128>()
        .map(TokenKind::Integer)
        .map_err(|_| Problem::not_built(line, "numbers beyond 128 bits are beyond the oracle"))
}

fn read_name(first: char, characters: &mut Peekable<Chars<'_>>) -> String {
    let mut name = String::from(first);
    while let Some(character) =
        characters.next_if(|character| character.is_ascii_alphanumeric() || *character == '_')
    {
        name.push(character);
    }
    name
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
