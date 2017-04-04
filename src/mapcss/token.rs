error_chain! {
    errors {
        LexerError(pos: InputPosition)
    }
}

use std::iter::Peekable;
use std::str::CharIndices;

pub type ZoomLevel = Option<u8>;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Token<'a> {
    Identifier(&'a str),
    String(&'a str),
    Number(f64),
    ZoomRange { min_zoom: ZoomLevel, max_zoom: ZoomLevel },
    Color { r: u8, g: u8, b: u8 },

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
    Dot,
    Colon,
    DoubleColon,
    SemiColon,
    Comma,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct InputPosition {
    pub line: usize,
    pub character: usize,
}

#[derive(Copy, Clone, Debug, PartialEq)]
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
            current_position: InputPosition {
                line: 1,
                character: 0,
            },
            had_newline: false,
        }
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
        } else if ch == '*' {
            Ok(Token::Identifier(&self.text[idx .. idx + 1]))
        } else if can_start_identifier(ch) {
            Ok(self.read_identifier(idx, ch))
        } else if ch == '"' {
            self.read_string(idx + ch.len_utf8())
        } else if ch == '-' || ch.to_digit(10).is_some() {
            self.read_number(ch)
        } else if ch == '|' {
            self.read_zoom_range()
        } else if ch == '#' {
            self.read_color()
        } else {
            bail!("Unexpected symbol: '{}'", ch)
        }
    }

    fn read_identifier(&mut self, start_idx: usize, ch: char) -> Token<'a> {
        let mut last_good_char_with_pos = (start_idx, ch);
        while let Some(&(next_idx, next_ch)) = self.chars.peek() {
            if can_continue_identifier(next_ch) {
                self.advance();
                last_good_char_with_pos = (next_idx, next_ch);
            } else {
                break;
            }
        }
        let (end_idx, last_char) = last_good_char_with_pos;
        Token::Identifier(&self.text[start_idx .. end_idx + last_char.len_utf8()])
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
            bail!("Unterminated string")
        } else {
            Ok(Token::String(&self.text[start_idx .. end_idx]))
        }
    }

    fn read_number(&mut self, mut first_ch: char) -> Result<Token<'a>> {
        let sign = if first_ch == '-' {
            match self.next_char() {
                Some(next_ch) => first_ch = next_ch,
                None => bail!("Expected a digit after '-'"),
            }
            -1.0_f64
        } else {
            1.0_f64
        };

        let mut number = match first_ch.to_digit(10) {
            Some(digit) => sign * (digit as f64),
            None => bail!("Expected a digit instead of '{}'", first_ch),
        };

        let mut had_dot = false;
        let mut digits_after_dot = 0;

        while let Some(next_ch) = self.peek_char() {
            if let Some(digit) = next_ch.to_digit(10) {
                let float_digit = digit as f64;
                if had_dot {
                    digits_after_dot += 1;
                    number += 10.0_f64.powi(-digits_after_dot) * float_digit;
                } else {
                    number = 10.0_f64 * number + float_digit;
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
            bail!("Expected a digit after '.'")
        } else {
            Ok(Token::Number(number))
        }
    }

    fn read_color(&mut self) -> Result<Token<'a>> {
        Ok(Token::Color {
            r: self.read_color_component()?,
            g: self.read_color_component()?,
            b: self.read_color_component()?,
        })
    }

    fn read_color_component(&mut self) -> Result<u8> {
        let mut read_hex_digit = || -> Result<u8> {
            match self.read_digit(16) {
                Some(digit) => Ok(digit),
                None => bail!("Expected a hexadecimal digit"),
            }
        };
        let digit1 = read_hex_digit()?;
        let digit2 = read_hex_digit()?;
        Ok(16 * digit1 + digit2)
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
            bail!("A zoom range should have either minumum or maximum level")
        } else {
            Ok(Token::ZoomRange {
                min_zoom: min_zoom,
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
            Some(ch) => {
                match ch.to_digit(radix) {
                    Some(digit) => {
                        self.advance();
                        Some(digit as u8)
                    },
                    None => None,
                }
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
                            _ => {},
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
            _ => bail!("Expected '{}' character", expected_ch),
        }
    }

    fn try_skip_comment(&mut self) -> Result<bool> {
        match self.peek_char() {
            Some('/') => {
                self.advance();
                self.skip_line_comment();
            },
            Some('*') => {
                self.advance();
                self.skip_block_comment()?;
            },
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
            match (ch, self.peek_char()) {
                ('*', Some('/')) => {
                    self.advance();
                    return Ok(());
                }
                _ => {},
            }
        }
        bail!("Unterminated block comment");
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Result<TokenWithPosition<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_significant_char().map(|x| {
            x
                .and_then(|(idx, ch)| {
                    let pos = self.current_position;
                    let token = self.read_token(idx, ch)?;
                    Ok(with_pos(token, pos))
                })
                .chain_err(|| ErrorKind::LexerError(self.current_position))
        })
    }
}

fn get_two_char_simple_token(fst: char, snd: char) -> Option<Token<'static>> {
    const TWO_LETTER_MATCH_TABLE: &'static [((char, char), Token<'static>)] = &[
        (('!', '='), Token::NotEqual),
        (('<', '='), Token::LessOrEqual),
        (('>', '='), Token::GreaterOrEqual),
        (('=', '~'), Token::RegexMatch),
        ((':', ':'), Token::DoubleColon),
    ];

    TWO_LETTER_MATCH_TABLE
        .iter()
        .filter_map(|&(x, token)|
            if x == (fst, snd) {
                Some(token)
            } else {
                None
            })
        .next()
}

fn get_one_char_simple_token(ch: char) -> Option<Token<'static>> {
    const ONE_LETTER_MATCH_TABLE: &'static [(char, Token<'static>)] = &[
        ('[', Token::LeftBracket),
        (']', Token::RightBracket),
        ('{', Token::LeftBrace),
        ('}', Token::RightBrace),
        ('=', Token::Equal),
        ('<', Token::Less),
        ('>', Token::Greater),
        ('!', Token::Bang),
        ('?', Token::QuestionMark),
        ('.', Token::Dot),
        (':', Token::Colon),
        (';', Token::SemiColon),
        (',', Token::Comma),
    ];

    ONE_LETTER_MATCH_TABLE
        .iter()
        .filter_map(|&(x, token)|
            if x == ch {
                Some(token)
            } else {
                None
            })
        .next()
}

fn can_start_identifier(ch: char) -> bool {
    match ch {
        '_' | 'a' ... 'z' | 'A' ... 'Z' => true,
        _ => false,
    }
}

fn can_continue_identifier(ch: char) -> bool {
    match ch {
        '-' | '0' ... '9' => true,
        ch if can_start_identifier(ch) => true,
        _ => false,
    }
}

fn with_pos<'a>(token: Token<'a>, position: InputPosition) -> TokenWithPosition<'a> {
    TokenWithPosition {
        token: token,
        position: position,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize<'a>(s: &'a str) -> Vec<Option<TokenWithPosition<'a>>> {
        Tokenizer::new(s)
            .map(|x| x.ok())
            .collect::<Vec<_>>()
    }

    fn unindent(s: &str) -> String {
        let lines = s.trim_matches('\n').split('\n').collect::<Vec<_>>();
        let space_count = lines[0].chars().take_while(|x| *x == ' ').count();
        lines.iter().map(|x| &x[space_count ..]).collect::<Vec<_>>().join("\n")
    }

    fn tok(s: &str, expected: Vec<Option<(Token, usize, usize)>>) {
        assert_eq!(tokenize(&unindent(s)), expected.iter().map(|x| x.map(|(token, line, ch)| {
            TokenWithPosition {
                token: token,
                position: InputPosition {
                    line: line,
                    character: ch,
                }
            }
        })).collect::<Vec<_>>())
    }

    #[test]
    fn test1() {
        tok(r#"
            /* this is a comment */
            way|z14-[highway=byway][bridge?],
            *::* {
                color: #ffcc00;
                dashes: 3,4;
                linejoin: round; // this is a comment, too
                width: 1.5;
                y-index: 4;
                z-index: -900;
            }
            "#,
        vec![
            Some((Token::Identifier("way"), 2, 1)),
            Some((Token::ZoomRange { min_zoom: Some(14), max_zoom: None }, 2, 4)),
            Some((Token::LeftBracket, 2, 9)),
            Some((Token::Identifier("highway"), 2, 10)),
            Some((Token::Equal, 2, 17)),
            Some((Token::Identifier("byway"), 2, 18)),
            Some((Token::RightBracket, 2, 23)),
            Some((Token::LeftBracket, 2, 24)),
            Some((Token::Identifier("bridge"), 2, 25)),
            Some((Token::QuestionMark, 2, 31)),
            Some((Token::RightBracket, 2, 32)),
            Some((Token::Comma, 2, 33)),
            Some((Token::Identifier("*"), 3, 1)),
            Some((Token::DoubleColon, 3, 2)),
            Some((Token::Identifier("*"), 3, 4)),
            Some((Token::LeftBrace, 3, 6)),
            Some((Token::Identifier("color"), 4, 5)),
            Some((Token::Colon, 4, 10)),
            Some((Token::Color { r: 255, g: 204, b: 0 }, 4, 12)),
            Some((Token::SemiColon, 4, 19)),
            Some((Token::Identifier("dashes"), 5, 5)),
            Some((Token::Colon, 5, 11)),
            Some((Token::Number(3.0), 5, 13)),
            Some((Token::Comma, 5, 14)),
            Some((Token::Number(4.0), 5, 15)),
            Some((Token::SemiColon, 5, 16)),
            Some((Token::Identifier("linejoin"), 6, 5)),
            Some((Token::Colon, 6, 13)),
            Some((Token::Identifier("round"), 6, 15)),
            Some((Token::SemiColon, 6, 20)),
            Some((Token::Identifier("width"), 7, 5)),
            Some((Token::Colon, 7, 10)),
            Some((Token::Number(1.5), 7, 12)),
            Some((Token::SemiColon, 7, 15)),
            Some((Token::Identifier("y-index"), 8, 5)),
            Some((Token::Colon, 8, 12)),
            Some((Token::Number(4.0), 8, 14)),
            Some((Token::SemiColon, 8, 15)),
            Some((Token::Identifier("z-index"), 9, 5)),
            Some((Token::Colon, 9, 12)),
            Some((Token::Number(-900.0), 9, 14)),
            Some((Token::SemiColon, 9, 18)),
            Some((Token::RightBrace, 10, 1)),
        ]);
    }
}
