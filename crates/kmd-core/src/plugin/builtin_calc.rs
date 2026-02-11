//! Built-in calculator extension
//!
//! Activated with `:calc` prefix or when the query looks like a math expression.
//! Supports basic arithmetic: +, -, *, /, %, parentheses.

use crate::index::{IndexItem, ItemKind, Source};
use super::{Extension, ExtensionAction};

pub struct CalcExtension;

impl Extension for CalcExtension {
    fn name(&self) -> &str {
        "Calculator"
    }

    fn prefix(&self) -> Option<&str> {
        Some(":calc")
    }

    fn search(&self, query: &str) -> Vec<IndexItem> {
        let expr = query.trim();
        if expr.is_empty() {
            return vec![IndexItem {
                name: "Calculator — type a math expression".to_string(),
                path: String::new(),
                kind: ItemKind::Calculator,
                source: Source::Plugin,
                icon: "\u{1F5A9}".to_string(), // 🖩
                keywords: "calculator calc math".to_string(),
            }];
        }

        match evaluate(expr) {
            Ok(result) => {
                let formatted = format_number(result);
                let display = format!("= {}", formatted);
                vec![IndexItem {
                    name: display,
                    path: formatted,
                    kind: ItemKind::Calculator,
                    source: Source::Plugin,
                    icon: "\u{1F5A9}".to_string(),
                    keywords: format!("calc {} {}", expr, result),
                }]
            }
            Err(_) => {
                vec![IndexItem {
                    name: format!("Invalid expression: {}", expr),
                    path: String::new(),
                    kind: ItemKind::Calculator,
                    source: Source::Plugin,
                    icon: "\u{274C}".to_string(), // ❌
                    keywords: String::new(),
                }]
            }
        }
    }

    fn execute(&self, item: &IndexItem) -> ExtensionAction {
        if item.path.is_empty() {
            ExtensionAction::Noop
        } else {
            ExtensionAction::CopyToClipboard(item.path.clone())
        }
    }
}

/// Check if a string looks like a math expression
pub fn looks_like_math(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || s.len() < 2 {
        return false;
    }
    // Must start with a digit or parenthesis
    let first = s.chars().next().unwrap();
    if !first.is_ascii_digit() && first != '(' && first != '-' {
        return false;
    }
    // Must contain at least one operator
    s.contains('+')
        || s.contains('-')
        || s.contains('*')
        || s.contains('/')
        || s.contains('%')
        || (s.contains('(') && s.contains(')'))
}

/// Format a number nicely with thousands separators
fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        let i = n as i64;
        format_with_commas(i)
    } else {
        format!("{:.6}", n)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// Add comma separators to an integer (e.g. 1234567 → "1,234,567")
fn format_with_commas(n: i64) -> String {
    let is_negative = n < 0;
    let s = n.unsigned_abs().to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    if is_negative {
        result.push('-');
    }
    result.chars().rev().collect()
}

/// Simple recursive descent expression evaluator
/// Supports: +, -, *, /, %, parentheses, unary minus
fn evaluate(expr: &str) -> Result<f64, CalcError> {
    let tokens = tokenize(expr)?;
    let mut pos = 0;
    let result = parse_expr(&tokens, &mut pos)?;
    if pos < tokens.len() {
        return Err(CalcError::UnexpectedToken);
    }
    Ok(result)
}

#[derive(Debug)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Mul,
    Div,
    Mod,
    LParen,
    RParen,
}

#[derive(Debug)]
enum CalcError {
    InvalidChar(char),
    UnexpectedToken,
    UnmatchedParen,
    DivisionByZero,
}

impl std::fmt::Display for CalcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChar(c) => write!(f, "Invalid character: {}", c),
            Self::UnexpectedToken => write!(f, "Unexpected token"),
            Self::UnmatchedParen => write!(f, "Unmatched parenthesis"),
            Self::DivisionByZero => write!(f, "Division by zero"),
        }
    }
}

fn tokenize(expr: &str) -> Result<Vec<Token>, CalcError> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' => {
                chars.next();
            }
            '0'..='9' | '.' => {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' || c == '_' {
                        if c != '_' {
                            num.push(c);
                        }
                        chars.next();
                    } else {
                        break;
                    }
                }
                let n: f64 = num.parse().map_err(|_| CalcError::InvalidChar('.'))?;
                tokens.push(Token::Number(n));
            }
            '+' => {
                tokens.push(Token::Plus);
                chars.next();
            }
            '-' => {
                tokens.push(Token::Minus);
                chars.next();
            }
            '*' => {
                tokens.push(Token::Mul);
                chars.next();
            }
            '/' => {
                tokens.push(Token::Div);
                chars.next();
            }
            '%' => {
                tokens.push(Token::Mod);
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            c => return Err(CalcError::InvalidChar(c)),
        }
    }

    Ok(tokens)
}

// expr = term (('+' | '-') term)*
fn parse_expr(tokens: &[Token], pos: &mut usize) -> Result<f64, CalcError> {
    let mut left = parse_term(tokens, pos)?;

    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Plus => {
                *pos += 1;
                left += parse_term(tokens, pos)?;
            }
            Token::Minus => {
                *pos += 1;
                left -= parse_term(tokens, pos)?;
            }
            _ => break,
        }
    }

    Ok(left)
}

// term = factor (('*' | '/' | '%') factor)*
fn parse_term(tokens: &[Token], pos: &mut usize) -> Result<f64, CalcError> {
    let mut left = parse_factor(tokens, pos)?;

    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Mul => {
                *pos += 1;
                left *= parse_factor(tokens, pos)?;
            }
            Token::Div => {
                *pos += 1;
                let right = parse_factor(tokens, pos)?;
                if right == 0.0 {
                    return Err(CalcError::DivisionByZero);
                }
                left /= right;
            }
            Token::Mod => {
                *pos += 1;
                let right = parse_factor(tokens, pos)?;
                if right == 0.0 {
                    return Err(CalcError::DivisionByZero);
                }
                left %= right;
            }
            _ => break,
        }
    }

    Ok(left)
}

// factor = ['-'] (NUMBER | '(' expr ')')
fn parse_factor(tokens: &[Token], pos: &mut usize) -> Result<f64, CalcError> {
    if *pos >= tokens.len() {
        return Err(CalcError::UnexpectedToken);
    }

    // Unary minus
    if matches!(tokens[*pos], Token::Minus) {
        *pos += 1;
        let val = parse_factor(tokens, pos)?;
        return Ok(-val);
    }

    // Unary plus
    if matches!(tokens[*pos], Token::Plus) {
        *pos += 1;
        return parse_factor(tokens, pos);
    }

    match &tokens[*pos] {
        Token::Number(n) => {
            let val = *n;
            *pos += 1;
            Ok(val)
        }
        Token::LParen => {
            *pos += 1;
            let val = parse_expr(tokens, pos)?;
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RParen) {
                return Err(CalcError::UnmatchedParen);
            }
            *pos += 1;
            Ok(val)
        }
        _ => Err(CalcError::UnexpectedToken),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        assert_eq!(evaluate("2 + 3").unwrap(), 5.0);
        assert_eq!(evaluate("10 - 3").unwrap(), 7.0);
        assert_eq!(evaluate("4 * 5").unwrap(), 20.0);
        assert_eq!(evaluate("15 / 3").unwrap(), 5.0);
        assert_eq!(evaluate("10 % 3").unwrap(), 1.0);
    }

    #[test]
    fn test_operator_precedence() {
        assert_eq!(evaluate("2 + 3 * 4").unwrap(), 14.0);
        assert_eq!(evaluate("(2 + 3) * 4").unwrap(), 20.0);
    }

    #[test]
    fn test_unary_minus() {
        assert_eq!(evaluate("-5").unwrap(), -5.0);
        assert_eq!(evaluate("-(3 + 2)").unwrap(), -5.0);
    }

    #[test]
    fn test_nested_parens() {
        assert_eq!(evaluate("((2 + 3) * (4 - 1))").unwrap(), 15.0);
    }

    #[test]
    fn test_division_by_zero() {
        assert!(evaluate("5 / 0").is_err());
    }

    #[test]
    fn test_looks_like_math() {
        assert!(looks_like_math("2 + 3"));
        assert!(looks_like_math("(1+2)*3"));
        assert!(!looks_like_math("hello"));
        assert!(!looks_like_math("5"));
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(42.0), "42");
        assert_eq!(format_number(3.14), "3.14");
        assert_eq!(format_number(0.1 + 0.2), "0.3");
        assert_eq!(format_number(1234.0), "1,234");
        assert_eq!(format_number(1234567.0), "1,234,567");
        assert_eq!(format_number(-99999.0), "-99,999");
    }

    #[test]
    fn test_calc_extension_inline() {
        let calc = CalcExtension;
        let results = <CalcExtension as crate::plugin::Extension>::search(&calc, "1234*1234");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "= 1,522,756");
        assert_eq!(results[0].path, "1,522,756");
    }
}
