use mapcss::errors::*;

use mapcss::color::Color;
use mapcss::token::{Token, TokenWithPosition, Tokenizer};

use std::fmt;

#[derive(Debug)]
pub enum ObjectType {
    All,
    Canvas,
    Meta,
    Node,
    Way { should_be_closed: Option<bool> },
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let object_type = match self {
            &ObjectType::All => "*",
            &ObjectType::Canvas => "canvas",
            &ObjectType::Meta => "meta",
            &ObjectType::Node => "node",
            &ObjectType::Way { should_be_closed } => match should_be_closed {
                None => "way",
                Some(true) => "area",
                Some(false) => "line",
            },
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
    Unary { tag_name: String, test_type: UnaryTestType },
    BinaryStringCompare { tag_name: String, value: String, test_type: BinaryStringTestType },
    BinaryNumericCompare { tag_name: String, value: f64, test_type: BinaryNumericTestType }
}

impl fmt::Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let quote = |tag_name: &String| {
            if tag_name.contains(":") {
                format!("\"{}\"", tag_name)
            } else {
                tag_name.clone()
            }
        };
        let result = match self {
            &Test::Unary { ref tag_name, ref test_type } => match test_type {
                &UnaryTestType::Exists => format!("{}", quote(tag_name)),
                &UnaryTestType::NotExists => format!("!{}", quote(tag_name)),
                &UnaryTestType::True => format!("{}?", quote(tag_name)),
                &UnaryTestType::False => format!("!{}?", quote(tag_name)),
            },
            &Test::BinaryStringCompare { ref tag_name, ref value, ref test_type } => {
                let sign = match test_type {
                    &BinaryStringTestType::Equal => "=",
                    &BinaryStringTestType::NotEqual => "!=",
                };
                format!("{}{}{}", quote(tag_name), sign, value)
            },
            &Test::BinaryNumericCompare { ref tag_name, ref value, ref test_type } => {
                let sign = match test_type {
                    &BinaryNumericTestType::Less => "<",
                    &BinaryNumericTestType::LessOrEqual => "<=",
                    &BinaryNumericTestType::Greater => ">",
                    &BinaryNumericTestType::GreaterOrEqual => ">=",
                };
                format!("{}{}{}", quote(tag_name), sign, value)
            },
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
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &PropertyValue::Color(Color { r, g, b }) => write!(f, "#{:02x}{:02x}{:02x}", r, g, b),
            &PropertyValue::Identifier(ref id) => write!(f, "{}", id),
            &PropertyValue::String(ref s) => write!(f, "\"{}\"", s),
            &PropertyValue::Numbers(ref nums) => write!(
                f,
                "{}",
                nums.iter().map(fmt_item::<f64>).collect::<Vec<_>>().join(",")
            )
        }
    }
}

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub value: PropertyValue,
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {};", self.name, self.value)
    }
}

#[derive(Debug)]
pub struct SingleSelector {
    pub object_type: ObjectType,
    pub min_zoom: Option<u8>,
    pub max_zoom: Option<u8>,
    pub tests: Vec<Test>,
    pub layer_id: Option<String>,
}

impl fmt::Display for SingleSelector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
            },
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
pub enum Selector {
    Single(SingleSelector),
    Nested { parent: SingleSelector, child: SingleSelector },
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Selector::Single(ref s) => write!(f, "{}", s),
            &Selector::Nested { ref parent, ref child } => write!(f, "{} > {}", parent, child),
        }
    }
}

#[derive(Debug)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub properties: Vec<Property>,
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {{\n{}\n}}",
            self.selectors.iter().map(fmt_item::<Selector>).collect::<Vec<_>>().join(",\n"),
            self.properties.iter().map(fmt_item::<Property>).collect::<Vec<_>>().join("\n")
        )
    }
}

fn fmt_item<T: fmt::Display>(item: &T) -> String {
    format!("{}", item)
}

pub struct Parser<'a> {
    tokenizer: Tokenizer<'a>,
}

enum ConsumedSelectorType {
    Ordinary,
    Parent,
    Last,
}

struct ConsumedSelector {
    selector: SingleSelector,
    selector_type: ConsumedSelectorType,
}

impl<'a> Parser<'a> {
    pub fn new(tokenizer: Tokenizer<'a>) -> Parser<'a> {
        Parser {
            tokenizer: tokenizer,
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Rule>> {
        let mut result = Vec::new();
        while let Some(rule) = self.read_rule()? {
            result.push(rule);
        }
        Ok(result)
    }

    fn read_rule(&mut self) -> Result<Option<Rule>> {
        let mut selector_start = match self.tokenizer.next() {
            None => return Ok(None),
            Some(token) => token?,
        };

        let mut rule = Rule {
            selectors: Vec::new(),
            properties: Vec::new(),
        };

        loop {
            let consumed_selector = self.read_selector(selector_start)?;

            let mut expect_more_selectors = match consumed_selector.selector_type {
                ConsumedSelectorType::Last => false,
                _ => true,
            };

            let selector_to_add = match consumed_selector.selector_type {
                ConsumedSelectorType::Ordinary | ConsumedSelectorType::Last => {
                    Selector::Single(consumed_selector.selector)
                },
                ConsumedSelectorType::Parent => {
                    let next_token = self.read_token()?;
                    let child_selector = self.read_selector(next_token)?;

                    match child_selector.selector_type {
                        ConsumedSelectorType::Parent => {
                            bail!(ErrorKind::ParseError(
                                String::from("A child selector can't be a parent to another selector"),
                                self.tokenizer.position()
                            ));
                        },
                        ConsumedSelectorType::Last => {
                            expect_more_selectors = false;
                        },
                        _ => {},
                    }

                    Selector::Nested {
                        parent: consumed_selector.selector,
                        child: child_selector.selector,
                    }
                },
            };

            rule.selectors.push(selector_to_add);
            if !expect_more_selectors {
                break;
            }
            selector_start = self.read_token()?;
        }

        rule.properties = self.read_properties()?;

        Ok(Some(rule))
    }

    fn read_selector(&mut self, selector_first_token: TokenWithPosition<'a>) -> Result<ConsumedSelector> {
        let mut selector = match selector_first_token.token {
            Token::Identifier(id) => {
                let object_type = id_to_object_type(id)
                    .ok_or_else(|| ErrorKind::ParseError(
                        format!("Unknown object type: {}", id),
                        selector_first_token.position
                    ))?;
                SingleSelector {
                    object_type: object_type,
                    min_zoom: None,
                    max_zoom: None,
                    tests: Vec::new(),
                    layer_id: None,
                }
            },
            _ => return self.unexpected_token(selector_first_token),
        };

        loop {
            let current_token = self.read_token()?;
            let mut consumed_selector_type = None;

            match current_token.token {
                Token::LeftBrace => {
                    consumed_selector_type = Some(ConsumedSelectorType::Last);
                },
                Token::Comma => {
                    consumed_selector_type = Some(ConsumedSelectorType::Ordinary);
                },
                Token::Greater => {
                    consumed_selector_type = Some(ConsumedSelectorType::Parent);
                },
                Token::ZoomRange { min_zoom, max_zoom } => {
                    selector.min_zoom = min_zoom;
                    selector.max_zoom = max_zoom;
                },
                Token::LeftBracket => {
                    selector.tests.push(self.read_test()?);
                },
                Token::Colon => {
                    // This is a pseudo-class. Even though we don't use them,
                    // we still have to parse them correctly.
                    self.read_identifier()?;
                },
                Token::DoubleColon => {
                    selector.layer_id = Some(self.read_identifier()?);
                }
                _ => return self.unexpected_token(current_token),
            }

            if let Some(selector_type) = consumed_selector_type {
                return Ok(ConsumedSelector {
                    selector: selector,
                    selector_type: selector_type,
                })
            }
        }
    }

    fn read_test(&mut self) -> Result<Test> {
        let mut starts_with_bang = false;

        let mut current_token = self.read_token()?;

        let lhs = match current_token.token {
            Token::Identifier(id) => String::from(id),
            Token::String(s) => String::from(s),
            Token::Bang => {
                starts_with_bang = true;
                self.read_identifier()?
            },
            _ => return self.unexpected_token(current_token),
        };

        current_token = self.read_token()?;

        if !starts_with_bang {
            if let Some(binary_op) = to_binary_string_test_type(current_token.token) {
                current_token = self.read_token()?;

                let rhs = match current_token.token {
                    Token::Identifier(id) => String::from(id),
                    Token::Number(num) => num.to_string(),
                    _ => return self.unexpected_token(current_token),
                };

                self.expect_simple_token(Token::RightBracket)?;

                return Ok(Test::BinaryStringCompare {
                    tag_name: lhs,
                    value: rhs,
                    test_type: binary_op,
                })
            }

            if let Some(binary_op) = to_binary_numeric_test_type(current_token.token) {
                current_token = self.read_token()?;

                let rhs = match current_token.token {
                    Token::Number(num) => num,
                    _ => return self.unexpected_token(current_token),
                };

                self.expect_simple_token(Token::RightBracket)?;

                return Ok(Test::BinaryNumericCompare {
                    tag_name: lhs,
                    value: rhs,
                    test_type: binary_op,
                });
            }
        }

        let unary_test_type = match current_token.token {
            Token::RightBracket => {
                if starts_with_bang { UnaryTestType::NotExists } else { UnaryTestType::Exists }
            },
            Token::QuestionMark => {
                current_token = self.read_token()?;
                match current_token.token {
                    Token::RightBracket => if starts_with_bang { UnaryTestType::False } else { UnaryTestType::True },
                    Token::Bang if !starts_with_bang => {
                        self.expect_simple_token(Token::RightBracket)?;
                        UnaryTestType::False
                    },
                    _ => return self.unexpected_token(current_token),
                }
            },
            _ => return self.unexpected_token(current_token),
        };

        Ok(Test::Unary {
            tag_name: lhs,
            test_type: unary_test_type,
        })
    }

    fn read_properties(&mut self) -> Result<Vec<Property>> {
        let mut result = Vec::new();
        loop {
            let token = self.read_token()?;
            match token.token {
                Token::Identifier(id) => {
                    self.expect_simple_token(Token::Colon)?;
                    result.push(Property {
                        name: String::from(id),
                        value: self.read_property_value()?,
                    })
                },
                Token::RightBrace => break,
                _ => return self.unexpected_token(token),
            }
        }
        Ok(result)
    }

    fn read_property_value(&mut self) -> Result<PropertyValue> {
        let token = self.read_token()?;
        let mut expect_semicolon = true;
        let result = match token.token {
            Token::Identifier(id) => PropertyValue::Identifier(String::from(id)),
            Token::String(s) => PropertyValue::String(String::from(s)),
            Token::Color(color) => PropertyValue::Color(color),
            Token::Number(num) => {
                expect_semicolon = false;
                PropertyValue::Numbers(self.read_number_list(num)?)
            },
            _ => return self.unexpected_token(token)?,
        };
        if expect_semicolon {
            self.expect_simple_token(Token::SemiColon)?;
        }
        Ok(result)
    }

    fn read_number_list(&mut self, first_num: f64) -> Result<Vec<f64>> {
        let mut numbers = vec![first_num];
        let mut consumed_number = true;
        loop {
            let next_token = self.read_token()?;
            match next_token.token {
                Token::Comma if consumed_number => {
                    consumed_number = false;
                },
                Token::SemiColon if consumed_number => break,
                Token::Number(next_num) if !consumed_number => {
                    consumed_number = true;
                    numbers.push(next_num);
                },
                _ => return self.unexpected_token(next_token),
            }
        }
        Ok(numbers)
    }

    fn read_identifier(&mut self) -> Result<String> {
        let token = self.read_token()?;
        match token.token {
            Token::Identifier(id) => Ok(String::from(id)),
            _ => self.unexpected_token(token),
        }
    }

    fn read_token(&mut self) -> Result<TokenWithPosition<'a>> {
        match self.tokenizer.next() {
            Some(token) => token.map_err(|x| From::from(x)),
            None => {
                bail!(ErrorKind::ParseError(String::from("Unexpected end of file"), self.tokenizer.position()))
            },
        }
    }

    fn expect_simple_token(&mut self, expected: Token<'static>) -> Result<()> {
        let token = self.read_token()?;
        if token.token != expected {
            bail!(ErrorKind::ParseError(format!("Expected '{}', found '{}' instead", expected, token.token), token.position))
        } else {
            Ok(())
        }
    }

    fn unexpected_token<T>(&self, token: TokenWithPosition<'a>) -> Result<T> {
        bail!(ErrorKind::ParseError(format!("Unexpected token: '{}'", token.token), token.position))
    }
}

fn id_to_object_type(id: &str) -> Option<ObjectType> {
    match id {
        "*" => Some(ObjectType::All),
        "canvas" => Some(ObjectType::Canvas),
        "meta" => Some(ObjectType::Meta),
        "node" => Some(ObjectType::Node),
        "way" => Some(ObjectType::Way {
            should_be_closed: None,
        }),
        "area" => Some(ObjectType::Way {
            should_be_closed: Some(true),
        }),
        "line" => Some(ObjectType::Way {
            should_be_closed: Some(false),
        }),
        _ => None,
    }
}

fn to_binary_string_test_type<'a>(token: Token<'a>) -> Option<BinaryStringTestType> {
    match token {
        Token::Equal => Some(BinaryStringTestType::Equal),
        Token::NotEqual => Some(BinaryStringTestType::NotEqual),
        _ => None,
    }
}

fn to_binary_numeric_test_type<'a>(token: Token<'a>) -> Option<BinaryNumericTestType> {
    match token {
        Token::Less => Some(BinaryNumericTestType::Less),
        Token::LessOrEqual => Some(BinaryNumericTestType::LessOrEqual),
        Token::Greater => Some(BinaryNumericTestType::Greater),
        Token::GreaterOrEqual => Some(BinaryNumericTestType::GreaterOrEqual),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;
    use std::io::{Read, Write};
    use std::path::PathBuf;

    #[test]
    fn test_mapnik_parse() {
        let mut mapnik_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for p in &["tests", "mapcss", "mapnik.mapcss"] {
            mapnik_path.push(p)
        }

        let mut mapnik_content = String::new();
        File::open(&mapnik_path).unwrap().read_to_string(&mut mapnik_content).unwrap();

        let tokenizer = Tokenizer::new(&mapnik_content);
        let mut parser = Parser::new(tokenizer);
        let rules = parser.parse().unwrap();

        let rules_str = rules.iter().map(fmt_item::<Rule>).collect::<Vec<_>>().join("\n\n");
        let mapnik_path_parsed = mapnik_path.with_extension("parsed");
        File::create(&mapnik_path_parsed).unwrap().write_all(rules_str.as_bytes()).unwrap();

        let mut canonical_rules_str = String::new();
        let mapnik_path_canonical = mapnik_path.with_extension("parsed.canonical");
        File::open(&mapnik_path_canonical).unwrap().read_to_string(&mut canonical_rules_str).unwrap();
        assert_eq!(rules_str, canonical_rules_str);
    }

    #[test]
    fn test_parsing_is_idempotent() {
        let mut mapnik_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for p in &["tests", "mapcss", "mapnik.parsed.canonical"] {
            mapnik_path.push(p)
        }

        let mut canonical = String::new();
        File::open(mapnik_path).unwrap().read_to_string(&mut canonical).unwrap();
        let mut parser = Parser::new(Tokenizer::new(&canonical));

        let rules_str = parser.parse().unwrap().iter().map(fmt_item::<Rule>).collect::<Vec<_>>().join("\n\n");
        assert_eq!(rules_str, canonical);
    }
}
