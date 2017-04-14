use mapcss::errors::*;

use mapcss::token::{Color, Token, TokenWithPosition, Tokenizer};

#[derive(Debug)]
pub enum ObjectType {
    All,
    Canvas,
    Meta,
    Node,
    Way { should_be_closed: Option<bool> },
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

#[derive(Debug)]
pub enum PropertyValue {
    Identifier(String),
    String(String),
    Numbers(Vec<f64>),
    Color(Color),
}

#[derive(Debug)]
pub struct Property {
    name: String,
    value: PropertyValue,
}

#[derive(Debug)]
pub struct SingleSelector {
    object_type: ObjectType,
    min_zoom: Option<u8>,
    max_zoom: Option<u8>,
    tests: Vec<Test>,
    layer_id: Option<String>,
}

#[derive(Debug)]
pub enum Selector {
    Single(SingleSelector),
    Nested { parent: SingleSelector, child: SingleSelector },
}

#[derive(Debug)]
pub struct Rule {
    selectors: Vec<Selector>,
    properties: Vec<Property>,
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

                self.expect_test_end()?;

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

                self.expect_test_end()?;

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
                        self.expect_test_end()?;
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

    fn expect_test_end(&mut self) -> Result<()> {
        let token = self.read_token()?;
        match token.token {
            Token::RightBracket => Ok(()),
            _ => bail!(ErrorKind::ParseError(format!("Expected ], found {} instead", token.token), token.position)),
        }
    }

    fn read_properties(&mut self) -> Result<Vec<Property>> {
        loop {
            let token = self.read_token()?;
            match token.token {
                Token::RightBrace => {
                    return Ok(Vec::new());
                },
                _ => {},
            }
        }
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

    fn unexpected_token<T>(&self, token: TokenWithPosition<'a>) -> Result<T> {
        bail!(ErrorKind::ParseError(format!("Unexpected token: {}", token.token), token.position))
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
