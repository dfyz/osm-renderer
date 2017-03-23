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
    Number(f64),
    ZoomRange(ZoomLevels),

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

        if ch == '*' {
            let pos = self.current_position;
            self.next_char();
            let identifier = Token::Identifier(&self.text[idx .. idx + 1]);
            Ok(with_pos(identifier, pos))
        } else if can_start_identifier(ch) {
            let pos = self.current_position;
            Ok(with_pos(self.read_identifier(idx, ch), pos))
        } else if ch == '"' {
            let pos = self.current_position;
            let string = self.read_string(idx + ch.len_utf8())?;
            Ok(with_pos(string, pos))
        } else {
            bail!("Unexpected symbol: {}", ch)
        }
    }

    fn read_identifier(&mut self, start_idx: usize, ch: char) -> Token<'a> {
        let mut last_good_char_with_pos = (start_idx, ch);
        while let Some(&(_, next_ch)) = self.chars.peek() {
            if can_continue_identifier(next_ch) {
                last_good_char_with_pos = self.next_char().unwrap();
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
        while let Some((next_idx, next_ch)) = self.next_char() {
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

    fn next_significant_char(&mut self) -> Option<Result<CharWithPos>> {
        loop {
            match self.next_char() {
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

    fn next_char(&mut self) -> Option<CharWithPos> {
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
        self.next_significant_char().map(|x| {
            x
                .and_then(|(idx, ch)| self.read_token(idx, ch))
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
        '-' | '_' | 'a' ... 'z' => true,
        _ => false,
    }
}

fn can_continue_identifier(ch: char) -> bool {
    match ch {
        '0' ... '9' => true,
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
