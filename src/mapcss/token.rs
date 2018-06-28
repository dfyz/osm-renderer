use mapcss::color::Color;
use mapcss::errors::*;

use std::fmt;
use std::iter::Peekable;
use std::str::CharIndices;

pub type ZoomLevel = Option<u8>;

#[derive(Clone, Debug, PartialEq)]
pub enum Token<'a> {
    Import(&'a str),
    Identifier(&'a str),
    String(&'a str),
    Number(f64),
    ZoomRange { min_zoom: ZoomLevel, max_zoom: ZoomLevel },
    ColorRef(&'a str),
    Color(Color),

    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,

    Equal,
    NotEqual,
    Less,
    Greater,
    LessOrEqual,
    GreaterOrEqual,
    RegexMatch,

    Bang,
    QuestionMark,
    Colon,
    DoubleColon,
    SemiColon,
    Comma,
}

const TWO_LETTER_MATCH_TABLE: &[((char, char), Token<'static>)] = &[
    (('!', '='), Token::NotEqual),
    (('<', '='), Token::LessOrEqual),
    (('>', '='), Token::GreaterOrEqual),
    (('=', '~'), Token::RegexMatch),
    ((':', ':'), Token::DoubleColon),
];

const ONE_LETTER_MATCH_TABLE: &[(char, Token<'static>)] = &[
    ('(', Token::LeftParen),
    (')', Token::RightParen),
    ('[', Token::LeftBracket),
    (']', Token::RightBracket),
    ('{', Token::LeftBrace),
    ('}', Token::RightBrace),
    ('=', Token::Equal),
    ('<', Token::Less),
    ('>', Token::Greater),
    ('!', Token::Bang),
    ('?', Token::QuestionMark),
    (':', Token::Colon),
    (';', Token::SemiColon),
    (',', Token::Comma),
];

impl<'a> fmt::Display for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for &((ch1, ch2), ref tok) in TWO_LETTER_MATCH_TABLE {
            if tok == self {
                return write!(f, "{}{}", ch1, ch2);
            }
        }
        for &(ch, ref tok) in ONE_LETTER_MATCH_TABLE {
            if tok == self {
                return write!(f, "{}", ch);
            }
        }

        write!(f, "{:?}", self)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct InputPosition {
    pub line: usize,
    pub character: usize,
}

impl fmt::Display for InputPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line {}, col {}", self.line, self.character)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TokenWithPosition<'a> {
    pub token: Token<'a>,
    pub position: InputPosition,
}

pub struct Tokenizer<'a> {
    text: &'a str,
    chars: Peekable<CharIndices<'a>>,
    current_position: InputPosition,
    had_newline: bool,
}

type CharWithPos = (usize, char);

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Tokenizer<'a> {
        Tokenizer {
            text: input,
            chars: input.char_indices().peekable(),
            current_position: InputPosition { line: 1, character: 0 },
            had_newline: false,
        }
    }

    pub fn position(&self) -> InputPosition {
        self.current_position
    }

    fn read_token(&mut self, idx: usize, ch: char) -> Result<Token<'a>> {
        if let Some(next_ch) = self.peek_char() {
            if let Some(token) = get_two_char_simple_token(ch, next_ch) {
                self.advance();
                return Ok(token);
            }
        }

        if let Some(token) = get_one_char_simple_token(ch) {
            Ok(token)
        } else if ch == '@' {
            self.read_at_directive()
        } else if ch == '*' {
            Ok(Token::Identifier(&self.text[idx..idx + 1]))
        } else if can_start_identifier(ch) {
            Ok(self.read_identifier(idx))
        } else if ch == '"' {
            self.read_string(idx + 1)
        } else if is_digit(ch) || ch == '+' || ch == '.' {
            self.read_number(ch)
        } else if ch == '-' {
            match self.peek_char() {
                Some(next_ch) if is_digit(next_ch) => self.read_number(ch),
                Some(next_ch) if can_continue_identifier(next_ch) => Ok(self.read_identifier(idx)),
                _ => self.lexer_error("Expected a valid number or identifier after '-'"),
            }
        } else if ch == '|' {
            self.read_zoom_range()
        } else if ch == '#' {
            self.read_color()
        } else {
            self.lexer_error(format!("Unexpected symbol: '{}'", ch))
        }
    }

    fn read_at_directive(&mut self) -> Result<Token<'a>> {
        let start_idx = match self.next_char_with_pos() {
            Some((idx, ch)) if can_be_in_at_directive(ch) => idx,
            _ => return self.lexer_error("Expected a letter or underscore after @"),
        };

        let mut end_idx = start_idx;
        while let Some(&(next_idx, next_ch)) = self.chars.peek() {
            if can_be_in_at_directive(next_ch) {
                self.advance();
                end_idx = next_idx;
            } else {
                break;
            }
        }

        let directive_text = &self.text[start_idx..end_idx + 1];
        if directive_text == "import" {
            self.expect_char('(')?;

            let import_text = match self.next_char_with_pos() {
                Some((idx, ch)) if ch == '"' => match self.read_string(idx + 1)? {
                    Token::String(text) => Ok(text),
                    _ => panic!("read_string() returned a non-string; this is a bug"),
                },
                _ => self.lexer_error("Expected a string"),
            }?;

            self.expect_char(')')?;
            Ok(Token::Import(import_text))
        } else {
            Ok(Token::ColorRef(directive_text))
        }
    }

    fn read_identifier(&mut self, start_idx: usize) -> Token<'a> {
        let mut end_idx = start_idx;
        while let Some(&(next_idx, next_ch)) = self.chars.peek() {
            if can_continue_identifier(next_ch) {
                self.advance();
                end_idx = next_idx;
            } else {
                break;
            }
        }
        Token::Identifier(&self.text[start_idx..end_idx + 1])
    }

    fn read_string(&mut self, start_idx: usize) -> Result<Token<'a>> {
        let mut end_idx = start_idx;
        let mut terminated_correctly = false;
        while let Some((next_idx, next_ch)) = self.next_char_with_pos() {
            end_idx = next_idx;
            if next_ch == '"' {
                terminated_correctly = true;
                break;
            }
        }
        if !terminated_correctly {
            self.lexer_error("Unterminated string")
        } else {
            Ok(Token::String(&self.text[start_idx..end_idx]))
        }
    }

    fn read_number(&mut self, mut first_ch: char) -> Result<Token<'a>> {
        let sign = match first_ch {
            '+' | '-' => match self.next_char() {
                Some(next_ch) => {
                    let res = if first_ch == '-' { -1.0 } else { 1.0 };
                    first_ch = next_ch;
                    res
                }
                None => return self.lexer_error("Expected a digit after '-' or '+'"),
            },
            _ => 1.0,
        };

        let mut had_dot = false;

        let mut number = match first_ch.to_digit(10) {
            Some(digit) => f64::from(digit),
            None => match first_ch {
                '.' => {
                    had_dot = true;
                    0.0
                }
                _ => return self.lexer_error(format!("Expected a digit or '.' instead of '{}'", first_ch)),
            },
        };

        let mut number_after_dot = 0.0f64;
        let mut digits_after_dot = 0;

        let add_digit = |current: &mut f64, digit| *current = 10.0_f64 * (*current) + f64::from(digit);

        while let Some(next_ch) = self.peek_char() {
            if let Some(digit) = next_ch.to_digit(10) {
                if had_dot {
                    digits_after_dot += 1;
                    add_digit(&mut number_after_dot, digit);
                } else {
                    add_digit(&mut number, digit);
                }
                self.advance();
            } else if next_ch == '.' && !had_dot {
                had_dot = true;
                self.advance();
            } else {
                break;
            }
        }

        if had_dot && (digits_after_dot == 0) {
            self.lexer_error("Expected a digit after '.'")
        } else {
            if digits_after_dot > 0 {
                number += number_after_dot / 10.0f64.powi(digits_after_dot)
            }
            Ok(Token::Number(sign * number))
        }
    }

    fn read_color(&mut self) -> Result<Token<'a>> {
        let mut color_digits = Vec::new();
        while let Some(hex_digit) = self.read_digit(16) {
            color_digits.push(hex_digit);
        }

        let read_component = |idx1, idx2| color_digits[idx1] * 16 + color_digits[idx2];

        let color = match color_digits.len() {
            6 => Color {
                r: read_component(0, 1),
                g: read_component(2, 3),
                b: read_component(4, 5),
            },
            3 => Color {
                r: read_component(0, 0),
                g: read_component(1, 1),
                b: read_component(2, 2),
            },
            _ => return self.lexer_error("Invalid hex color (expected #RGB or #RRGGBB)"),
        };

        Ok(Token::Color(color))
    }

    fn read_zoom_range(&mut self) -> Result<Token<'a>> {
        self.expect_char('z')?;
        let min_zoom = self.read_zoom_level();
        let had_hyphen = {
            if let Some('-') = self.peek_char() {
                self.advance();
                true
            } else {
                false
            }
        };
        let max_zoom = self.read_zoom_level();

        if min_zoom.is_none() && max_zoom.is_none() {
            self.lexer_error("A zoom range should have either minumum or maximum level")
        } else {
            Ok(Token::ZoomRange {
                min_zoom,
                max_zoom: if had_hyphen { max_zoom } else { min_zoom },
            })
        }
    }

    fn read_zoom_level(&mut self) -> ZoomLevel {
        match self.read_digit(10) {
            Some(num1) => match self.read_digit(10) {
                Some(num2) => Some(10 * num1 + num2),
                None => Some(num1),
            },
            None => None,
        }
    }

    fn read_digit(&mut self, radix: u32) -> Option<u8> {
        match self.peek_char() {
            Some(ch) => match ch.to_digit(radix) {
                Some(digit) => {
                    self.advance();
                    Some(digit as u8)
                }
                None => None,
            },
            _ => None,
        }
    }

    fn next_significant_char(&mut self) -> Option<Result<CharWithPos>> {
        loop {
            match self.next_char_with_pos() {
                None => return None,
                Some((idx, ch)) => {
                    if ch.is_whitespace() {
                        continue;
                    }
                    if ch == '/' {
                        match self.try_skip_comment() {
                            Err(err) => return Some(Err(err)),
                            Ok(true) => continue,
                            _ => {}
                        }
                    }
                    return Some(Ok((idx, ch)));
                }
            }
        }
    }

    fn next_char_with_pos(&mut self) -> Option<CharWithPos> {
        let res = self.chars.next();

        if self.had_newline {
            self.current_position.line += 1;
            self.current_position.character = 0;
            self.had_newline = false;
        }

        self.current_position.character += 1;
        self.had_newline = match res {
            Some((_, '\n')) => true,
            _ => false,
        };

        res
    }

    fn next_char(&mut self) -> Option<char> {
        self.next_char_with_pos().map(|x| x.1)
    }

    fn advance(&mut self) {
        self.next_char();
    }

    fn peek_char(&mut self) -> Option<char> {
        self.chars.peek().map(|x| x.1)
    }

    fn expect_char(&mut self, expected_ch: char) -> Result<()> {
        match self.next_char() {
            Some(actual_ch) if actual_ch == expected_ch => Ok(()),
            _ => self.lexer_error(format!("Expected '{}' character", expected_ch)),
        }
    }

    fn try_skip_comment(&mut self) -> Result<bool> {
        match self.peek_char() {
            Some('/') => {
                self.advance();
                self.skip_line_comment();
            }
            Some('*') => {
                self.advance();
                self.skip_block_comment()?;
            }
            _ => {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn skip_line_comment(&mut self) {
        while let Some(ch) = self.next_char() {
            if ch == '\n' {
                return;
            }
        }
    }

    fn skip_block_comment(&mut self) -> Result<()> {
        while let Some(ch) = self.next_char() {
            if let ('*', Some('/')) = (ch, self.peek_char()) {
                self.advance();
                return Ok(());
            }
        }
        self.lexer_error("Unterminated block comment")
    }

    fn lexer_error<T, Msg: Into<String>>(&self, message: Msg) -> Result<T> {
        bail!(ErrorKind::LexerError(message.into(), self.current_position))
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Result<TokenWithPosition<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_significant_char().map(|x| {
            x.and_then(|(idx, ch)| {
                let pos = self.current_position;
                let token = self.read_token(idx, ch)?;
                Ok(with_pos(token, pos))
            })
        })
    }
}

fn get_two_char_simple_token(fst: char, snd: char) -> Option<Token<'static>> {
    TWO_LETTER_MATCH_TABLE
        .iter()
        .filter_map(|&(x, ref token)| if x == (fst, snd) { Some(token.clone()) } else { None })
        .next()
}

fn get_one_char_simple_token(ch: char) -> Option<Token<'static>> {
    ONE_LETTER_MATCH_TABLE
        .iter()
        .filter_map(|&(x, ref token)| if x == ch { Some(token.clone()) } else { None })
        .next()
}

fn can_be_in_at_directive(ch: char) -> bool {
    match ch {
        '_' | 'a'...'z' | '0'...'9' => true,
        _ => false,
    }
}

fn can_start_identifier(ch: char) -> bool {
    match ch {
        '_' | 'a'...'z' | 'A'...'Z' => true,
        _ => false,
    }
}

fn can_continue_identifier(ch: char) -> bool {
    match ch {
        '-' | '0'...'9' | '.' | '/' => true,
        ch if can_start_identifier(ch) => true,
        _ => false,
    }
}

fn is_digit(ch: char) -> bool {
    ch.to_digit(10).is_some()
}

fn with_pos(token: Token, position: InputPosition) -> TokenWithPosition {
    TokenWithPosition { token, position }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize<'a>(s: &'a str) -> Vec<TokenWithPosition<'a>> {
        Tokenizer::new(s)
            .map(|x| x.expect("Unexpected lexer error"))
            .collect::<Vec<_>>()
    }

    fn unindent(s: &str) -> String {
        let lines = s.trim_matches('\n').split('\n').collect::<Vec<_>>();
        let space_count = lines[0].chars().take_while(|x| *x == ' ').count();
        lines.iter().map(|x| &x[space_count..]).collect::<Vec<_>>().join("\n")
    }

    fn tok(s: &str, expected: Vec<(Token, usize, usize)>) {
        assert_eq!(
            tokenize(&unindent(s)),
            expected
                .into_iter()
                .map(|(ref token, line, ch)| TokenWithPosition {
                    token: token.clone(),
                    position: InputPosition { line, character: ch },
                })
                .collect::<Vec<_>>()
        )
    }

    #[test]
    fn test1() {
        tok(
            r#"
            /* this is a comment */
            way|z14-[highway=byway][bridge?],
            *::* {
                color: @black;
                dashes: 3,4;
                linejoin: round; // this is a comment, too
                width: 1.5;
                y-index: 4;
                z-index: -999;
            }
            @import("include.mapcss");
            @black: #ffcc00;
            "#,
            vec![
                (Token::Identifier("way"), 2, 1),
                (
                    Token::ZoomRange {
                        min_zoom: Some(14),
                        max_zoom: None,
                    },
                    2,
                    4,
                ),
                (Token::LeftBracket, 2, 9),
                (Token::Identifier("highway"), 2, 10),
                (Token::Equal, 2, 17),
                (Token::Identifier("byway"), 2, 18),
                (Token::RightBracket, 2, 23),
                (Token::LeftBracket, 2, 24),
                (Token::Identifier("bridge"), 2, 25),
                (Token::QuestionMark, 2, 31),
                (Token::RightBracket, 2, 32),
                (Token::Comma, 2, 33),
                (Token::Identifier("*"), 3, 1),
                (Token::DoubleColon, 3, 2),
                (Token::Identifier("*"), 3, 4),
                (Token::LeftBrace, 3, 6),
                (Token::Identifier("color"), 4, 5),
                (Token::Colon, 4, 10),
                (Token::ColorRef("black"), 4, 12),
                (Token::SemiColon, 4, 18),
                (Token::Identifier("dashes"), 5, 5),
                (Token::Colon, 5, 11),
                (Token::Number(3.0), 5, 13),
                (Token::Comma, 5, 14),
                (Token::Number(4.0), 5, 15),
                (Token::SemiColon, 5, 16),
                (Token::Identifier("linejoin"), 6, 5),
                (Token::Colon, 6, 13),
                (Token::Identifier("round"), 6, 15),
                (Token::SemiColon, 6, 20),
                (Token::Identifier("width"), 7, 5),
                (Token::Colon, 7, 10),
                (Token::Number(1.5), 7, 12),
                (Token::SemiColon, 7, 15),
                (Token::Identifier("y-index"), 8, 5),
                (Token::Colon, 8, 12),
                (Token::Number(4.0), 8, 14),
                (Token::SemiColon, 8, 15),
                (Token::Identifier("z-index"), 9, 5),
                (Token::Colon, 9, 12),
                (Token::Number(-999.0), 9, 14),
                (Token::SemiColon, 9, 18),
                (Token::RightBrace, 10, 1),
                (Token::Import("include.mapcss"), 11, 1),
                (Token::SemiColon, 11, 26),
                (Token::ColorRef("black"), 12, 1),
                (Token::Colon, 12, 7),
                (Token::Color(Color { r: 255, g: 204, b: 0 }), 12, 9),
                (Token::SemiColon, 12, 16),
            ],
        );
    }

    #[test]
    fn test2() {
        tok(
            r#"
            line|z12-14[piste:lift=j-bar],
            line|z12-14[piste:lift=magic_carpet],
            line|z19-[power=line],
            way|z-16[highway=secondary]
            {width: 2.5;opacity: 0.6;dashes: 0.9,18;}
            "#,
            vec![
                (Token::Identifier("line"), 1, 1),
                (
                    Token::ZoomRange {
                        min_zoom: Some(12),
                        max_zoom: Some(14),
                    },
                    1,
                    5,
                ),
                (Token::LeftBracket, 1, 12),
                (Token::Identifier("piste"), 1, 13),
                (Token::Colon, 1, 18),
                (Token::Identifier("lift"), 1, 19),
                (Token::Equal, 1, 23),
                (Token::Identifier("j-bar"), 1, 24),
                (Token::RightBracket, 1, 29),
                (Token::Comma, 1, 30),
                (Token::Identifier("line"), 2, 1),
                (
                    Token::ZoomRange {
                        min_zoom: Some(12),
                        max_zoom: Some(14),
                    },
                    2,
                    5,
                ),
                (Token::LeftBracket, 2, 12),
                (Token::Identifier("piste"), 2, 13),
                (Token::Colon, 2, 18),
                (Token::Identifier("lift"), 2, 19),
                (Token::Equal, 2, 23),
                (Token::Identifier("magic_carpet"), 2, 24),
                (Token::RightBracket, 2, 36),
                (Token::Comma, 2, 37),
                (Token::Identifier("line"), 3, 1),
                (
                    Token::ZoomRange {
                        min_zoom: Some(19),
                        max_zoom: None,
                    },
                    3,
                    5,
                ),
                (Token::LeftBracket, 3, 10),
                (Token::Identifier("power"), 3, 11),
                (Token::Equal, 3, 16),
                (Token::Identifier("line"), 3, 17),
                (Token::RightBracket, 3, 21),
                (Token::Comma, 3, 22),
                (Token::Identifier("way"), 4, 1),
                (
                    Token::ZoomRange {
                        min_zoom: None,
                        max_zoom: Some(16),
                    },
                    4,
                    4,
                ),
                (Token::LeftBracket, 4, 9),
                (Token::Identifier("highway"), 4, 10),
                (Token::Equal, 4, 17),
                (Token::Identifier("secondary"), 4, 18),
                (Token::RightBracket, 4, 27),
                (Token::LeftBrace, 5, 1),
                (Token::Identifier("width"), 5, 2),
                (Token::Colon, 5, 7),
                (Token::Number(2.5), 5, 9),
                (Token::SemiColon, 5, 12),
                (Token::Identifier("opacity"), 5, 13),
                (Token::Colon, 5, 20),
                (Token::Number(0.6), 5, 22),
                (Token::SemiColon, 5, 25),
                (Token::Identifier("dashes"), 5, 26),
                (Token::Colon, 5, 32),
                (Token::Number(0.9), 5, 34),
                (Token::Comma, 5, 37),
                (Token::Number(18.0), 5, 38),
                (Token::SemiColon, 5, 40),
                (Token::RightBrace, 5, 41),
            ],
        );
    }

    #[test]
    fn test3() {
        tok(
            r#"
            node|z14-[railway=signal]["railway:signal:direction"]["railway:signal:speed_limit_distant:deactivated"=yes]::deactivatedcross
            {
                icon-image: "icons/light-signal-deactivated-18.png";
                text-allow-overlap: true;
            }
            "#,
            vec![
                (Token::Identifier("node"), 1, 1),
                (
                    Token::ZoomRange {
                        min_zoom: Some(14),
                        max_zoom: None,
                    },
                    1,
                    5,
                ),
                (Token::LeftBracket, 1, 10),
                (Token::Identifier("railway"), 1, 11),
                (Token::Equal, 1, 18),
                (Token::Identifier("signal"), 1, 19),
                (Token::RightBracket, 1, 25),
                (Token::LeftBracket, 1, 26),
                (Token::String("railway:signal:direction"), 1, 27),
                (Token::RightBracket, 1, 53),
                (Token::LeftBracket, 1, 54),
                (Token::String("railway:signal:speed_limit_distant:deactivated"), 1, 55),
                (Token::Equal, 1, 103),
                (Token::Identifier("yes"), 1, 104),
                (Token::RightBracket, 1, 107),
                (Token::DoubleColon, 1, 108),
                (Token::Identifier("deactivatedcross"), 1, 110),
                (Token::LeftBrace, 2, 1),
                (Token::Identifier("icon-image"), 3, 5),
                (Token::Colon, 3, 15),
                (Token::String("icons/light-signal-deactivated-18.png"), 3, 17),
                (Token::SemiColon, 3, 56),
                (Token::Identifier("text-allow-overlap"), 4, 5),
                (Token::Colon, 4, 23),
                (Token::Identifier("true"), 4, 25),
                (Token::SemiColon, 4, 29),
                (Token::RightBrace, 5, 1),
            ],
        )
    }

    #[test]
    fn test_errors() {
        let malformed_strings = ["/*abc", "-", "123.", "\"abc", "|z-", "#", "&", "+"];
        for s in &malformed_strings {
            let errors = Tokenizer::new(s).collect::<Vec<_>>();
            assert_eq!(1, errors.len(), "Expected exactly one error for {}", s);
            assert!(errors[0].is_err(), "Expected to have an error for {}", s);
        }
    }
}
