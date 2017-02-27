pub type ZoomLevel = Option<u8>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ZoomLevels {
    pub min_zoom: ZoomLevel,
    pub max_zoom: ZoomLevel,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Tok<'a> {
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
    Dot,
    Colon,
    DoubleColon,
    SemiColon,
}

// Grouped by the first symbol, sorted by decreasing length in each group
// to make sure we always capture the longest token.
const SIMPLE_TOKEN_MATCH_TABLE: &'static [(&'static str, Tok<'static>)] = &[
    // The unambigous tokens form the first group.
    ("[", Tok::LeftBracket),
    ("]", Tok::RightBracket),
    ("{", Tok::LeftBrace),
    ("}", Tok::RightBrace),
    (".", Tok::Dot),
    (";", Tok::SemiColon),

    ("=~", Tok::RegexMatch),
    ("=", Tok::Equal),

    ("!=", Tok::NotEqual),
    ("!", Tok::Bang),

    ("<=", Tok::LessOrEqual),
    ("<", Tok::Less),

    (">=", Tok::GreaterOrEqual),
    (">", Tok::Greater),

    ("::", Tok::DoubleColon),
    (":", Tok::Colon),
];

type TokenResult<'a> = Result<(usize, Tok<'a>, usize), String>;

pub struct Tokenizer<'a> {
    text: &'a str,
    chars: Vec<(usize, char)>,
    char_index: usize,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Tokenizer<'a> {
        Tokenizer {
            text: input,
            chars: input.char_indices().collect(),
            char_index: 0,
        }
    }

    fn consume(&mut self, token: Tok<'a>, char_count: usize) -> TokenResult<'a> {
        let result = (self.char_index, token, self.char_index + char_count);
        self.char_index += char_count;
        Ok(result)
    }

    fn get_current_char(&self) -> char {
        self.chars[self.char_index].1
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = TokenResult<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let None = self.chars.get(self.char_index) {
            return None;
        }

        for &(ref simple_token, ref token_type) in SIMPLE_TOKEN_MATCH_TABLE {
            let has_match = simple_token
                .chars()
                .enumerate()
                .all(|(offset, ch)| {
                    let text_index = self.char_index + offset;
                    match self.chars.get(text_index) {
                        Some(&(_, text_ch)) if ch == text_ch => true,
                        _ => false,
                    }
                });
            if has_match {
                return Some(self.consume(*token_type, simple_token.chars().count()));
            }
        }

        Some(Err(format!("Unrecognized symbol: {}", self.get_current_char())))
    }
}
