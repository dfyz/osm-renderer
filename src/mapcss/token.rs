error_chain! {
    errors {
        LexerError(pos: InputPosition)
    }
}

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

    fn read_token_or_eof(&mut self) -> Result<Option<TokenWithPosition<'a>>> {
        if let Some((idx, ch)) = self.next_significant_char()? {
            let token = self.read_token(idx, ch)?;
            Ok(Some(token))
        } else {
            Ok(None)
        }
    }

    fn read_token(&mut self, idx: usize, ch: char) -> Result<TokenWithPosition<'a>> {
        if let Some(&(_, next_ch)) = self.chars.peek() {
            if let Some(token) = get_two_char_simple_token(ch, next_ch) {
                self.next_char();
                return Ok(with_pos(token, self.current_position));
            }
        }
        if let Some(token) = get_one_char_simple_token(ch) {
            return Ok(with_pos(token, self.current_position));
        }
        bail!("Unexpected symbol: {}", ch);
    }

    fn next_significant_char(&mut self) -> Result<Option<(usize, char)>> {
        loop {
            let idx_ch = self.next_char();
            match idx_ch {
                None => return Ok(None),
                Some((_, ch)) => {
                    if ch.is_whitespace() {
                        continue;
                    }
                    if ch == '/' && self.try_skip_comment()? {
                        continue;
                    }
                    return Ok(idx_ch);
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
        let token_or_err = self
            .read_token_or_eof()
            .chain_err(|| ErrorKind::LexerError(self.current_position));
        match token_or_err {
            Ok(None) => None,
            Ok(Some(token)) => Some(Ok(token)),
            Err(err) => Some(Err(err)),
        }
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

fn with_pos<'a>(token: Token<'a>, position: InputPosition) -> TokenWithPosition<'a> {
    TokenWithPosition {
        token: token,
        position: position,
    }
}
