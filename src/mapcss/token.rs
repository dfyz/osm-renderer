use errors::*;

use std::iter::Peekable;
use std::str::CharIndices;

pub type ZoomLevel = Option<u8>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ZoomLevels {
    pub min_zoom: ZoomLevel,
    pub max_zoom: ZoomLevel,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Token<'a> {
    Identifier(&'a str),
    String(&'a str),
    Regex(&'a str),
    Number(f64),
    ZoomRange(ZoomLevels),
    Eval(&'a str),

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

    Eof,
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

    fn next_significant_char(&mut self) -> Option<Result<(usize, char)>> {
        loop {
            let idx_ch = self.next_char();
            match idx_ch {
                None => return None,
                Some((_, ch)) => {
                    if ch.is_whitespace() {
                        continue;
                    }
                    if ch == '/' {
                        match self.try_skip_comment() {
                            Err(e) => {
                                return Some(Err(e));
                            },
                            Ok(true) => {
                                continue;
                            },
                            Ok(false) => {},
                        }
                    }
                    return idx_ch.map(|x| Ok(x));
                },
            }
        }
    }

    fn next_char(&mut self) -> Option<(usize, char)> {
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

    fn try_skip_comment(&mut self) -> Result<bool> {
        match self.chars.peek() {
            Some(&(_, '/')) => {
                self.next_char();
                self.skip_line_comment();
            },
            Some(&(_, '*')) => {
                self.next_char();
                self.skip_block_comment()?;
            },
            _ => {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn skip_line_comment(&mut self) {
        while let Some((_, ch)) = self.next_char() {
            if ch == '\n' {
                return;
            }
        }
    }

    fn skip_block_comment(&mut self) -> Result<()> {
        while let Some((_, ch)) = self.next_char() {
            match (ch, self.chars.peek()) {
                ('*', Some(&(_, '/'))) => {
                    self.next_char();
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
        self
            .next_significant_char()
            .map(|x| x.and_then(
                |(idx, ch)| Ok(TokenWithPosition {
                    token: Token::String(&self.text[idx .. idx + ch.len_utf8()]),
                    position: self.current_position,
                })
            ))
    }
}

// Grouped by the first symbol, sorted by decreasing length in each group
// to make sure we always capture the longest token.
const SIMPLE_TOKEN_MATCH_TABLE: &'static [(&'static str, Token<'static>)] = &[
    // The unambigous tokens form the first group.
    ("[", Token::LeftBracket),
    ("]", Token::RightBracket),
    ("{", Token::LeftBrace),
    ("}", Token::RightBrace),
    (".", Token::Dot),
    (";", Token::SemiColon),
    ("?", Token::QuestionMark),

    ("=~", Token::RegexMatch),
    ("=", Token::Equal),

    ("!=", Token::NotEqual),
    ("!", Token::Bang),

    ("<=", Token::LessOrEqual),
    ("<", Token::Less),

    (">=", Token::GreaterOrEqual),
    (">", Token::Greater),

    ("::", Token::DoubleColon),
    (":", Token::Colon),
];