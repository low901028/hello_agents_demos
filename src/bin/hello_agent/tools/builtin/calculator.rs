use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;
use std::collections::HashMap;

pub struct CalculatorTool;

impl CalculatorTool {
    pub fn new() -> Self {
        CalculatorTool
    }

    fn eval_expression(expr: &str) -> Result<f64, String> {
        let expr = expr
            .trim()
            .replace("pi", &std::f64::consts::PI.to_string())
            .replace("e", &std::f64::consts::E.to_string());
        let tokens = Self::tokenize(&expr)?;
        PrattParser::new(tokens).parse(0)
    }

    fn tokenize(expr: &str) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = expr.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                c if c.is_whitespace() => {
                    i += 1;
                }
                c if c.is_ascii_digit() || c == '.' => {
                    let start = i;
                    while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                        i += 1;
                    }
                    tokens.push(Token::Num(
                        expr[start..i].parse().map_err(|_| "无效数字".to_string())?,
                    ));
                }
                '+' => {
                    tokens.push(Token::Add);
                    i += 1;
                }
                '-' => {
                    tokens.push(Token::Sub);
                    i += 1;
                }
                '*' => {
                    if i + 1 < chars.len() && chars[i + 1] == '*' {
                        tokens.push(Token::Pow);
                        i += 2;
                    } else {
                        tokens.push(Token::Mul);
                        i += 1;
                    }
                }
                '/' => {
                    tokens.push(Token::Div);
                    i += 1;
                }
                '(' => {
                    tokens.push(Token::LParen);
                    i += 1;
                }
                ')' => {
                    tokens.push(Token::RParen);
                    i += 1;
                }
                c if c.is_alphabetic() => {
                    let start = i;
                    while i < chars.len() && chars[i].is_alphabetic() {
                        i += 1;
                    }
                    tokens.push(Token::Func(chars[start..i].iter().collect()));
                }
                _ => return Err(format!("无效字符: {}", chars[i])),
            }
        }
        tokens.push(Token::EOF);
        Ok(tokens)
    }
}

impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "python_calculator"
    }
    fn description(&self) -> &str {
        "执行数学计算，支持基本运算和数学函数"
    }
    fn run(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        let expr = parameters
            .get("input")
            .or(parameters.get("expression"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if expr.is_empty() {
            return ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "表达式不能为空");
        }
        match Self::eval_expression(expr) {
            Ok(result) => {
                let mut data = HashMap::new();
                data.insert("result".into(), serde_json::json!(result));
                data.insert("expression".into(), serde_json::json!(expr));
                ToolResponse::success(format!("计算结果: {}", result), data)
            }
            Err(e) => ToolResponse::error(ToolErrorCode::ExecutionError.as_str(), &e),
        }
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new("input", "string", "数学表达式")]
    }
}

pub fn calculate(expr: &str) -> String {
    let mut p = HashMap::new();
    p.insert("input".into(), serde_json::json!(expr));
    CalculatorTool.run(p).text
}

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    LParen,
    RParen,
    Func(String),
    Comma,
    EOF,
}

impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Token::Num(a), Token::Num(b)) => a.to_bits() == b.to_bits(),
            (Token::Func(a), Token::Func(b)) => a == b,
            (Token::Add, Token::Add)
            | (Token::Sub, Token::Sub)
            | (Token::Mul, Token::Mul)
            | (Token::Div, Token::Div)
            | (Token::Pow, Token::Pow)
            | (Token::LParen, Token::LParen)
            | (Token::RParen, Token::RParen)
            | (Token::Comma, Token::Comma)
            | (Token::EOF, Token::EOF) => true,
            _ => false,
        }
    }
}

struct PrattParser {
    tokens: Vec<Token>,
    pos: usize,
}

impl PrattParser {
    fn new(tokens: Vec<Token>) -> Self {
        PrattParser { tokens, pos: 0 }
    }
    fn cur(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn adv(&mut self) -> &Token {
        self.pos += 1;
        &self.tokens[self.pos - 1]
    }

    fn parse(&mut self, min_prec: u8) -> Result<f64, String> {
        let mut left = self.prefix()?;
        while self.precedence() >= min_prec {
            let op = self.cur().clone();
            self.adv();
            left = self.infix(left, &op)?;
        }
        Ok(left)
    }

    fn precedence(&self) -> u8 {
        match self.cur() {
            Token::Add | Token::Sub => 1,
            Token::Mul | Token::Div => 2,
            Token::Pow => 3,
            _ => 0,
        }
    }

    fn prefix(&mut self) -> Result<f64, String> {
        match self.cur().clone() {
            Token::Num(n) => {
                self.adv();
                Ok(n)
            }
            Token::Sub => {
                self.adv();
                Ok(-self.parse(4)?)
            }
            Token::LParen => {
                self.adv();
                let v = self.parse(0)?;
                self.adv();
                Ok(v)
            }
            Token::Func(name) => {
                self.adv();
                self.adv();
                let arg = self.parse(0)?;
                self.adv();
                match name.as_str() {
                    "sqrt" => Ok(arg.sqrt()),
                    "sin" => Ok(arg.sin()),
                    "cos" => Ok(arg.cos()),
                    "tan" => Ok(arg.tan()),
                    "log" => Ok(arg.ln()),
                    "exp" => Ok(arg.exp()),
                    "abs" => Ok(arg.abs()),
                    _ => Err(format!("未知函数:{}", name)),
                }
            }
            _ => Err(format!("意外的token: {:?}", self.cur())),
        }
    }

    fn infix(&mut self, left: f64, op: &Token) -> Result<f64, String> {
        let prec = self.precedence();
        self.adv();
        let right = self.parse(prec + 1)?;
        match op {
            Token::Add => Ok(left + right),
            Token::Sub => Ok(left - right),
            Token::Mul => Ok(left * right),
            Token::Div => {
                if right == 0.0 {
                    Err("除零".into())
                } else {
                    Ok(left / right)
                }
            }
            Token::Pow => Ok(left.powf(right)),
            _ => Err("无效操作符".into()),
        }
    }
}
