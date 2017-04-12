use mapcss::token::{Color, InputPosition, Token, TokenWithPosition, Tokenizer};

error_chain! {
    links {
        LexerError(::mapcss::token::Error, ::mapcss::token::ErrorKind);
    }

    errors {
        ParserError(pos: InputPosition)
    }
}

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
    True,
    False,
}

#[derive(Debug)]
pub enum BinaryTestType {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

#[derive(Debug)]
pub enum Test {
    Unary { tag_name: String, test_type: UnaryTestType },
    BinaryOp { tag_name: String, value: String, test_type: BinaryTestType },
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
pub struct Selector {
    object_type: ObjectType,
    min_zoom_range: Option<u8>,
    max_zoom_range: Option<u8>,
    tests: Vec<Test>,
    layer_id: Option<String>,
}

#[derive(Debug)]
pub struct Rule {
    selectors: Vec<Selector>,
    properties: Vec<Property>,
}

pub struct Parser<'a> {
    tokenizer: Tokenizer<'a>,
}

struct ConsumedSelector {
    selector: Selector,
    last_selector: bool,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Parser<'a> {
        Parser {
            tokenizer: Tokenizer::new(input),
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Rule>> {
        let mut result = Vec::new();
        while let Some(token) = self.tokenizer.next() {
            let ok_token = token?;
            let rule = self.read_rule(ok_token)?;
            result.push(rule);
        }
        Ok(result)
    }

    fn read_rule(&mut self, rule_start: TokenWithPosition<'a>) -> Result<Rule> {
        let mut rule = Rule {
            selectors: Vec::new(),
            properties: Vec::new(),
        };

        let mut selector_start = rule_start;
        loop {
            let consumed_selector = self.read_selector(selector_start)?;
            if consumed_selector.last_selector {
                break;
            }
            rule.selectors.push(consumed_selector.selector);
            selector_start = self.read_token()?;
        }

        rule.properties = self.read_properties()?;

        Ok(rule)
    }

    fn read_selector(&mut self, selector_first_token: TokenWithPosition<'a>) -> Result<ConsumedSelector> {
        let selector = match selector_first_token.token {
            Token::Identifier(id) => {
                let object_type = id_to_object_type(id).chain_err(|| ErrorKind::ParserError(selector_first_token.position))?;
                Selector {
                    object_type: object_type,
                    min_zoom_range: None,
                    max_zoom_range: None,
                    tests: Vec::new(),
                    layer_id: None,
                }
            },
            _ => {
                return self.unexpected_token(selector_first_token);
            },
        };

        loop {
            let current_token = self.read_token()?;
            let mut selector_ended = false;
            let mut last_selector = false;

            match current_token.token {
                Token::LeftBrace => {
                    selector_ended = true;
                    last_selector = true;
                },
                Token::Comma => {
                    selector_ended = true;
                },
                _ => {},
            }

            if selector_ended {
                return Ok(ConsumedSelector {
                    selector: selector,
                    last_selector: last_selector,
                })
            }
        }
    }

    fn read_properties(&mut self) -> Result<Vec<Property>> {
        loop {
            let next_token = self.read_token()?;
            match next_token.token {
                Token::RightBrace => {
                    return Ok(Vec::new());
                },
                _ => {},
            }
        }
    }

    fn read_token(&mut self) -> Result<TokenWithPosition<'a>> {
        match self.tokenizer.next() {
            Some(token) => token.map_err(|x| From::from(x)),
            None => {
                let msg: Result<TokenWithPosition<'a>> =
                    Err("Unexpected end of file".into());
                msg.chain_err(|| ErrorKind::ParserError(self.tokenizer.position()))
            },
        }
    }

    fn unexpected_token<T>(&self, token: TokenWithPosition<'a>) -> Result<T> {
        let msg: Result<T> =
            Err(format!("Unexpected token: {}", token.token).into());
        msg.chain_err(|| ErrorKind::ParserError(token.position))
    }
}

fn id_to_object_type(id: &str) -> Result<ObjectType> {
    match id {
        "*" => Ok(ObjectType::All),
        "canvas" => Ok(ObjectType::Canvas),
        "meta" => Ok(ObjectType::Meta),
        "node" => Ok(ObjectType::Node),
        "way" => Ok(ObjectType::Way {
            should_be_closed: None,
        }),
        "area" => Ok(ObjectType::Way {
            should_be_closed: Some(true),
        }),
        "line" => Ok(ObjectType::Way {
            should_be_closed: Some(false),
        }),
        _ => bail!(format!("Unknown object type: {}", id)),
    }
}
