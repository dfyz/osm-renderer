use crate::mapcss::color::Color;
use crate::mapcss::token::{InputPosition, Token, TokenWithPosition, Tokenizer};
use crate::mapcss::MapcssError;

use anyhow::{Context, Error, Result};
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ObjectType {
    All,
    Canvas,
    Meta,
    Node,
    Way,
    Area,
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let object_type = match *self {
            ObjectType::All => "*",
            ObjectType::Canvas => "canvas",
            ObjectType::Meta => "meta",
            ObjectType::Node => "node",
            ObjectType::Way => "way",
            ObjectType::Area => "area",
        };
        write!(f, "{}", object_type)
    }
}

#[derive(Debug)]
pub enum UnaryTestType {
    Exists,
    NotExists,
    True,
    False,
}

#[derive(Debug)]
pub enum BinaryStringTestType {
    Equal,
    NotEqual,
}

#[derive(Debug)]
pub enum BinaryNumericTestType {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

#[derive(Debug)]
pub enum Test {
    Unary {
        tag_name: String,
        test_type: UnaryTestType,
    },
    BinaryStringCompare {
        tag_name: String,
        value: String,
        test_type: BinaryStringTestType,
    },
    BinaryNumericCompare {
        tag_name: String,
        value: f64,
        test_type: BinaryNumericTestType,
    },
}

impl fmt::Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let quote = |tag_name: &String| {
            if tag_name.contains(':') {
                format!("\"{}\"", tag_name)
            } else {
                tag_name.clone()
            }
        };
        let result = match *self {
            Test::Unary {
                ref tag_name,
                ref test_type,
            } => match *test_type {
                UnaryTestType::Exists => quote(tag_name),
                UnaryTestType::NotExists => format!("!{}", quote(tag_name)),
                UnaryTestType::True => format!("{}?", quote(tag_name)),
                UnaryTestType::False => format!("!{}?", quote(tag_name)),
            },
            Test::BinaryStringCompare {
                ref tag_name,
                ref value,
                ref test_type,
            } => {
                let sign = match *test_type {
                    BinaryStringTestType::Equal => "=",
                    BinaryStringTestType::NotEqual => "!=",
                };
                format!("{}{}{}", quote(tag_name), sign, value)
            }
            Test::BinaryNumericCompare {
                ref tag_name,
                ref value,
                ref test_type,
            } => {
                let sign = match *test_type {
                    BinaryNumericTestType::Less => "<",
                    BinaryNumericTestType::LessOrEqual => "<=",
                    BinaryNumericTestType::Greater => ">",
                    BinaryNumericTestType::GreaterOrEqual => ">=",
                };
                format!("{}{}{}", quote(tag_name), sign, value)
            }
        };
        write!(f, "[{}]", result)
    }
}

#[derive(Debug)]
pub enum PropertyValue {
    Identifier(String),
    String(String),
    Color(Color),
    Numbers(Vec<f64>),
    WidthDelta(f64),
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            PropertyValue::Color(Color { r, g, b }) => write!(f, "#{:02x}{:02x}{:02x}", r, g, b),
            PropertyValue::Identifier(ref id) => write!(f, "{}", id),
            PropertyValue::String(ref s) => write!(f, "\"{}\"", s),
            PropertyValue::Numbers(ref nums) => {
                write!(f, "{}", nums.iter().map(fmt_item::<f64>).collect::<Vec<_>>().join(","))
            }
            PropertyValue::WidthDelta(ref delta) => write!(f, "eval(prop(\"width\")) + {}", delta),
        }
    }
}

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub value: PropertyValue,
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {};", self.name, self.value)
    }
}

#[derive(Debug)]
pub struct Selector {
    pub object_type: ObjectType,
    pub min_zoom: Option<u8>,
    pub max_zoom: Option<u8>,
    pub tests: Vec<Test>,
    pub layer_id: Option<String>,
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let formatted_zoom_range = match (self.min_zoom, self.max_zoom) {
            (None, None) => String::new(),
            (Some(mn), None) => format!("{}-", mn),
            (None, Some(mx)) => format!("-{}", mx),
            (Some(mn), Some(mx)) => {
                if mn != mx {
                    format!("{}-{}", mn, mx)
                } else {
                    format!("{}", mn)
                }
            }
        };
        let formatted_layer_id = match self.layer_id {
            Some(ref id) => format!("::{}", id),
            None => String::new(),
        };
        write!(
            f,
            "{}{}{}{}{}",
            self.object_type,
            if formatted_zoom_range.is_empty() { "" } else { "|z" },
            formatted_zoom_range,
            self.tests.iter().map(fmt_item::<Test>).collect::<Vec<_>>().join(""),
            formatted_layer_id
        )
    }
}

#[derive(Debug)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub properties: Vec<Property>,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {{\n{}\n}}",
            self.selectors
                .iter()
                .map(fmt_item::<Selector>)
                .collect::<Vec<_>>()
                .join(",\n"),
            self.properties
                .iter()
                .map(fmt_item::<Property>)
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

pub fn parse_file(base_path: &Path, file_name: &str) -> Result<Vec<Rule>> {
    let content = read_stylesheet(base_path, file_name)?;
    let mut parser = Parser {
        tokenizer: Tokenizer::new(&content),
        base_path: base_path.to_owned(),
        file_name: file_name.to_string(),
        color_defs: ColorDefs::default(),
    };
    parser.parse()
}

type ColorDefs = HashMap<String, Color>;

struct Parser<'a> {
    tokenizer: Tokenizer<'a>,
    base_path: PathBuf,
    file_name: String,
    color_defs: ColorDefs,
}

impl<'a> Parser<'a> {
    pub fn parse(&mut self) -> Result<Vec<Rule>> {
        let mut result = Vec::new();
        loop {
            match self.read_optional_token() {
                None => break,
                Some(token_or_err) => {
                    let token = token_or_err?;
                    match token.token {
                        Token::Import(imported_file) => {
                            self.expect_simple_token(&Token::SemiColon)?;
                            let (rules, color_defs) = self.import_file(imported_file)?;
                            result.extend(rules);
                            self.color_defs.extend(color_defs);
                        }
                        Token::ColorRef(color_name) => self.read_color_def(color_name)?,
                        _ => result.push(self.read_rule(token)?),
                    }
                }
            }
        }
        Ok(result)
    }

    fn import_file(&mut self, file_name: &str) -> Result<(Vec<Rule>, ColorDefs)> {
        let content = read_stylesheet(&self.base_path, file_name)?;
        let mut parser = Parser {
            tokenizer: Tokenizer::new(&content),
            base_path: self.base_path.clone(),
            file_name: file_name.to_string(),
            color_defs: self.color_defs.clone(),
        };
        let imported_rules = parser.parse()?;
        Ok((imported_rules, parser.color_defs))
    }

    fn read_color_def(&mut self, color_name: &str) -> Result<()> {
        self.expect_simple_token(&Token::Colon)?;
        let color_value = {
            let color_value_token = self.read_mandatory_token()?;
            match color_value_token.token {
                Token::Color(color) => Some(color),
                // Don't add unknown values to the color definitions,
                // but don't fail the parsing process either.
                _ => None,
            }
        };
        self.expect_simple_token(&Token::SemiColon)?;
        if let Some(val) = color_value {
            self.color_defs.insert(color_name.to_string(), val);
        }
        Ok(())
    }

    fn read_rule(&mut self, mut selector_start: TokenWithPosition<'a>) -> Result<Rule> {
        let mut rule = Rule {
            selectors: Vec::new(),
            properties: Vec::new(),
        };

        loop {
            if let Token::LeftBrace = selector_start.token {
                break;
            }

            // Ignore the auxiliary section from Maps.ME MapCSS.
            if let Token::Identifier("colors") = selector_start.token {
                loop {
                    if let Token::RightBrace = self.read_mandatory_token()?.token {
                        break;
                    }
                }
                return Ok(rule);
            }

            let consumed_selector = self.read_selector(&selector_start)?;
            rule.selectors.push(consumed_selector.selector);
            if !consumed_selector.expect_more_selectors {
                break;
            }
            selector_start = self.read_mandatory_token()?;
        }

        rule.properties = self.read_properties()?;

        Ok(rule)
    }

    fn read_selector(&mut self, selector_first_token: &TokenWithPosition<'a>) -> Result<ConsumedSelector> {
        let mut selector = match selector_first_token.token {
            Token::Identifier(id) => {
                let object_type = id_to_object_type(id).ok_or_else(|| {
                    self.parse_error(format!("Unknown object type: {}", id), selector_first_token.position)
                })?;
                Selector {
                    object_type,
                    min_zoom: None,
                    max_zoom: None,
                    tests: Vec::new(),
                    layer_id: None,
                }
            }
            _ => return self.unexpected_token(selector_first_token),
        };

        loop {
            let current_token = self.read_mandatory_token()?;
            let mut expect_more_selectors = None;

            match current_token.token {
                Token::LeftBrace => {
                    expect_more_selectors = Some(false);
                }
                Token::Comma => {
                    expect_more_selectors = Some(true);
                }
                Token::ZoomRange { min_zoom, max_zoom } => {
                    selector.min_zoom = min_zoom;
                    selector.max_zoom = max_zoom;
                }
                Token::LeftBracket => {
                    selector.tests.push(self.read_test()?);
                }
                Token::Colon => {
                    // This is a pseudo-class. Even though we don't use them,
                    // we still have to parse them correctly.
                    self.read_identifier()?;
                }
                Token::DoubleColon => {
                    selector.layer_id = Some(self.read_identifier()?);
                }
                _ => return self.unexpected_token(&current_token),
            }

            if let Some(expect_more_selectors) = expect_more_selectors {
                return Ok(ConsumedSelector {
                    selector,
                    expect_more_selectors,
                });
            }
        }
    }

    fn read_test(&mut self) -> Result<Test> {
        let mut starts_with_bang = false;

        let mut current_token = self.read_mandatory_token()?;

        let mut lhs = match current_token.token {
            Token::Identifier(id) => String::from(id),
            Token::String(s) => String::from(s),
            Token::Bang => {
                starts_with_bang = true;
                self.read_identifier()?
            }
            _ => return self.unexpected_token(&current_token),
        };

        current_token = self.read_mandatory_token()?;

        if let Token::Colon = current_token.token {
            lhs.push(':');
            lhs.push_str(&self.read_identifier()?);
            current_token = self.read_mandatory_token()?;
        }

        if !starts_with_bang {
            if let Some(binary_op) = to_binary_string_test_type(&current_token.token) {
                current_token = self.read_mandatory_token()?;

                let rhs = match current_token.token {
                    Token::Identifier(id) => String::from(id),
                    Token::Number(num) => num.to_string(),
                    _ => return self.unexpected_token(&current_token),
                };

                self.expect_simple_token(&Token::RightBracket)?;

                return Ok(Test::BinaryStringCompare {
                    tag_name: lhs,
                    value: rhs,
                    test_type: binary_op,
                });
            }

            if let Some(binary_op) = to_binary_numeric_test_type(&current_token.token) {
                current_token = self.read_mandatory_token()?;

                let rhs = match current_token.token {
                    Token::Number(num) => num,
                    _ => return self.unexpected_token(&current_token),
                };

                self.expect_simple_token(&Token::RightBracket)?;

                return Ok(Test::BinaryNumericCompare {
                    tag_name: lhs,
                    value: rhs,
                    test_type: binary_op,
                });
            }
        }

        let unary_test_type = match current_token.token {
            Token::RightBracket => {
                if starts_with_bang {
                    UnaryTestType::NotExists
                } else {
                    UnaryTestType::Exists
                }
            }
            Token::QuestionMark => {
                current_token = self.read_mandatory_token()?;
                match current_token.token {
                    Token::RightBracket => {
                        if starts_with_bang {
                            UnaryTestType::False
                        } else {
                            UnaryTestType::True
                        }
                    }
                    Token::Bang if !starts_with_bang => {
                        self.expect_simple_token(&Token::RightBracket)?;
                        UnaryTestType::False
                    }
                    _ => return self.unexpected_token(&current_token),
                }
            }
            _ => return self.unexpected_token(&current_token),
        };

        Ok(Test::Unary {
            tag_name: lhs,
            test_type: unary_test_type,
        })
    }

    fn read_properties(&mut self) -> Result<Vec<Property>> {
        let mut result = Vec::new();
        loop {
            let token = self.read_mandatory_token()?;
            match token.token {
                Token::Identifier(id) => {
                    self.expect_simple_token(&Token::Colon)?;
                    result.push(Property {
                        name: String::from(id),
                        value: self.read_property_value()?,
                    });
                }
                Token::RightBrace => break,
                _ => return self.unexpected_token(&token),
            }
        }
        Ok(result)
    }

    fn read_property_value(&mut self) -> Result<PropertyValue> {
        let token = self.read_mandatory_token()?;
        let mut expect_semicolon = true;
        let result = match token.token {
            Token::Identifier(id) => {
                expect_semicolon = false;
                match id {
                    "eval" => self.read_simple_eval(token.position)?,
                    _ => {
                        let mut full_id = id.to_string();
                        let token = self.read_mandatory_token()?;
                        match token.token {
                            Token::Colon => {
                                full_id.push(':');
                                full_id.push_str(&self.read_identifier()?);
                                self.expect_simple_token(&Token::SemiColon)?;
                            }
                            Token::SemiColon => {}
                            _ => return self.unexpected_token(&token),
                        }
                        PropertyValue::Identifier(full_id)
                    }
                }
            }
            Token::String(s) => PropertyValue::String(String::from(s)),
            Token::Color(color) => PropertyValue::Color(color),
            Token::ColorRef(color_name) => match self.color_defs.get(color_name) {
                Some(color) => PropertyValue::Color(color.clone()),
                None => {
                    return Err(self.parse_error(
                        format!("Unknown color reference: {}", color_name),
                        self.tokenizer.position(),
                    ));
                }
            },
            Token::Number(num) => {
                expect_semicolon = false;
                PropertyValue::Numbers(self.read_number_list(num)?)
            }
            _ => return self.unexpected_token(&token)?,
        };
        if expect_semicolon {
            self.expect_simple_token(&Token::SemiColon)?;
        }
        Ok(result)
    }

    // Support the only form of eval() used in Maps.ME: eval(prop("width") + X);
    fn read_simple_eval(&mut self, position: InputPosition) -> Result<PropertyValue> {
        let mut tokens = Vec::new();
        loop {
            let token = self.read_mandatory_token()?;
            match token.token {
                Token::SemiColon => break,
                token => tokens.push(token),
            }
        }
        let expected_prefix = [
            Token::LeftParen,
            Token::Identifier("prop"),
            Token::LeftParen,
            Token::String("width"),
            Token::RightParen,
        ];
        let width_increment = {
            if !tokens.starts_with(&expected_prefix) {
                None
            } else {
                let suffix = &tokens[expected_prefix.len()..];
                if !suffix.is_empty() && suffix.last().unwrap() == &Token::RightParen {
                    match suffix.len() {
                        1 => Some(0.0),
                        2 => match suffix[suffix.len() - 2] {
                            Token::Number(num) => Some(num),
                            _ => None,
                        },
                        _ => None,
                    }
                } else {
                    None
                }
            }
        };

        match width_increment {
            Some(num) => Ok(PropertyValue::WidthDelta(num)),
            _ => Err(self.parse_error("Unknown eval(...) form", position)),
        }
    }

    fn read_number_list(&mut self, first_num: f64) -> Result<Vec<f64>> {
        let mut numbers = vec![first_num];
        let mut consumed_number = true;
        loop {
            let next_token = self.read_mandatory_token()?;
            match next_token.token {
                Token::Comma if consumed_number => {
                    consumed_number = false;
                }
                Token::SemiColon if consumed_number => break,
                Token::Number(next_num) if !consumed_number => {
                    consumed_number = true;
                    numbers.push(next_num);
                }
                _ => return self.unexpected_token(&next_token),
            }
        }
        Ok(numbers)
    }

    fn read_identifier(&mut self) -> Result<String> {
        let token = self.read_mandatory_token()?;
        match token.token {
            Token::Identifier(id) => Ok(String::from(id)),
            _ => self.unexpected_token(&token),
        }
    }

    fn read_mandatory_token(&mut self) -> Result<TokenWithPosition<'a>> {
        match self.read_optional_token() {
            Some(token) => token,
            None => Err(self.parse_error("Unexpected end of file", self.tokenizer.position())),
        }
    }

    fn read_optional_token(&mut self) -> Option<Result<TokenWithPosition<'a>>> {
        self.tokenizer.next().map(|x| {
            x.context(format!("Failed to tokenize {}", self.file_name))
                .map_err(Error::from)
        })
    }

    fn expect_simple_token(&mut self, expected: &Token<'static>) -> Result<()> {
        let token = self.read_mandatory_token()?;
        if token.token != *expected {
            Err(self.parse_error(
                format!("Expected '{}', found '{}' instead", expected, token.token),
                token.position,
            ))
        } else {
            Ok(())
        }
    }

    fn unexpected_token<T>(&self, token: &TokenWithPosition<'a>) -> Result<T> {
        Err(self.parse_error(format!("Unexpected token: '{}'", token.token), token.position))
    }

    fn parse_error<Msg: Into<String>>(&self, message: Msg, position: InputPosition) -> Error {
        Error::from(MapcssError::ParseError {
            message: message.into(),
            pos: position,
            file_name: self.file_name.clone(),
        })
    }
}

fn read_stylesheet(base_path: &Path, file_name: &str) -> Result<String> {
    let file_path = base_path.join(file_name);
    let mut stylesheet_reader = File::open(file_path).context("Failed to open the stylesheet file")?;
    let mut stylesheet = String::new();
    stylesheet_reader
        .read_to_string(&mut stylesheet)
        .context("Failed to read the stylesheet file")?;
    Ok(stylesheet)
}

fn id_to_object_type(id: &str) -> Option<ObjectType> {
    match id {
        "*" => Some(ObjectType::All),
        "canvas" => Some(ObjectType::Canvas),
        "meta" => Some(ObjectType::Meta),
        "node" => Some(ObjectType::Node),
        "way" | "line" => Some(ObjectType::Way),
        "area" => Some(ObjectType::Area),
        _ => None,
    }
}

struct ConsumedSelector {
    selector: Selector,
    expect_more_selectors: bool,
}

fn to_binary_string_test_type(token: &Token<'_>) -> Option<BinaryStringTestType> {
    match *token {
        Token::Equal => Some(BinaryStringTestType::Equal),
        Token::NotEqual => Some(BinaryStringTestType::NotEqual),
        _ => None,
    }
}

fn to_binary_numeric_test_type(token: &Token<'_>) -> Option<BinaryNumericTestType> {
    match *token {
        Token::Less => Some(BinaryNumericTestType::Less),
        Token::LessOrEqual => Some(BinaryNumericTestType::LessOrEqual),
        Token::Greater => Some(BinaryNumericTestType::Greater),
        Token::GreaterOrEqual => Some(BinaryNumericTestType::GreaterOrEqual),
        _ => None,
    }
}

fn fmt_item<T: fmt::Display>(item: &T) -> String {
    format!("{}", item)
}
