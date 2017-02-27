use std::str::CharIndices;

pub type ZoomLevel = Option<u8>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ZoomLevels {
    pub min_zoom: ZoomLevel,
    pub max_zoom: ZoomLevel,
}

#[derive(Clone, Debug, PartialEq)]
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
    LeftParens,
    RightParens,

    Equal,
    NotEqual,
    Less,
    Greater,
    LessOrEqual,
    GreaterOrEqual,
    RegexMatch,

    Bang,
    Dot,
    Hyphen,
    Colon,
    DoubleColon,
    SemiColon,
}

pub struct Tokenizer<'a> {
    text: &'a str,
    chars: CharIndices<'a>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Tokenizer<'a> {
        Tokenizer{
            text: input,
            chars: input.char_indices(),
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = (usize, Tok<'a>, usize);

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
