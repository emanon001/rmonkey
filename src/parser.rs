use crate::ast::{self};
use crate::lexer::Lexer;
use crate::token::Token;
use itertools::Itertools;
use std::collections::BTreeMap;

pub fn parse(lexer: Lexer) -> std::result::Result<ast::Program, Errors> {
    let mut parser = Parser::new(lexer);
    parser.parse()
}

// Precedence

#[derive(Debug, PartialEq, Eq, Copy, Clone, PartialOrd)]
enum Precedence {
    Lowest,
    Equals,
    LessGreater,
    Sum,     // + or -
    Product, // * or /
    Prefix,
    Call,
    Index,
}

// Erros

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Errors(pub Vec<String>);

impl std::fmt::Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.0.iter().join("\n");
        write!(f, "{}", s)
    }
}
impl std::error::Error for Errors {}

// Result
// for inner parse
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// Parser

struct Parser {
    lexer: std::iter::Peekable<Lexer>,
    current_token: Option<Token>,
}

impl Parser {
    fn new(lexer: Lexer) -> Self {
        let mut iter = lexer.into_iter().peekable();
        let current_token = iter.next();
        Self {
            lexer: iter,
            current_token,
        }
    }

    fn next(&mut self) {
        self.current_token = self.lexer.next();
    }

    fn current_token(&self) -> Option<&Token> {
        self.current_token.as_ref()
    }

    fn peek_token(&mut self) -> Option<&Token> {
        self.lexer.peek()
    }

    fn parse(&mut self) -> std::result::Result<ast::Program, Errors> {
        let mut statements = Vec::new();
        let mut errors = Vec::new();
        while self.current_token().is_some() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => errors.push(format!("{}", e)),
            }
            self.next();
        }
        if errors.is_empty() {
            Ok(ast::Program { statements })
        } else {
            Err(Errors(errors))
        }
    }

    fn parse_statement(&mut self) -> Result<ast::Statement> {
        match self.current_token() {
            Some(Token::Let) => self.parse_let_statement(),
            Some(Token::Return) => self.parse_return_statement(),
            Some(_) => self.parse_expression_statement(),
            t => Err(Self::new_parse_error("statement", t).into()),
        }
    }

    fn parse_let_statement(&mut self) -> Result<ast::Statement> {
        // let <identifier> = <expression>;

        // let
        self.expect_current_token(Token::Let)?;
        self.next();

        // <identifier>
        let identifier = Self::parse_identifier(self.current_token())?;

        // =
        self.expect_peek_token_and_next(Token::Assign)?;
        self.next();

        // <expression>
        let expression = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token() == Some(&Token::Semicolon) {
            self.next();
        }

        Ok(ast::Statement::Let {
            identifier,
            expression,
        })
    }

    fn parse_return_statement(&mut self) -> Result<ast::Statement> {
        // return <expression>;

        // return
        self.expect_current_token(Token::Return)?;
        self.next();

        // <expression>
        let expression = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token() == Some(&Token::Semicolon) {
            self.next();
        }

        Ok(ast::Statement::Return(expression))
    }

    fn parse_expression_statement(&mut self) -> Result<ast::Statement> {
        // `<expression>` | `<expression>;`
        assert!(self.current_token().is_some());

        let expr = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token() == Some(&Token::Semicolon) {
            self.next();
        }
        let stmt = ast::Statement::Expression(expr);
        Ok(stmt)
    }

    fn parse_block_statement(&mut self) -> Result<ast::BlockStatement> {
        self.expect_current_token(Token::LBrace)?;
        self.next();
        let mut statements = Vec::new();
        while self
            .current_token()
            .filter(|&t| t != &Token::RBrace)
            .is_some()
        {
            let s = self.parse_statement()?;
            statements.push(s);
            self.next();
        }
        Ok(ast::BlockStatement { statements })
    }

    fn parse_expression(&mut self, precedence: Precedence) -> Result<ast::Expression> {
        // <expression>

        // parse prefix
        let mut left = self.parse_prefix()?;

        // parse infix
        while self
            .peek_token()
            .filter(|&t| t != &Token::Semicolon)
            .is_some()
            && precedence < self.peek_prececence()
        {
            let (expr, parsed) = self.parse_infix(left)?;
            if !parsed {
                return Ok(expr);
            }
            left = expr;
        }
        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<ast::Expression> {
        match self.current_token() {
            Some(Token::Identifier(_)) => self.parse_identifier_expression(),
            Some(Token::Int(_)) => self.parse_integer_expression(),
            Some(Token::True) | Some(Token::False) => self.parse_boolean_expression(),
            Some(Token::String(_)) => self.parse_string_expression(),
            Some(Token::LBracket) => self.parse_array_expression(),
            Some(Token::LBrace) => self.parse_hash_expression(),
            Some(Token::Bang) | Some(Token::Minus) => self.parse_prefix_expression(),
            Some(Token::LParen) => self.parse_grouped_expression(),
            Some(Token::If) => self.parse_if_expression(),
            Some(Token::Function) => self.parse_function_expression(),
            Some(Token::Quote) => self.parse_quote_expression(),
            Some(Token::Unquote) => self.parse_unquote_expression(),
            Some(Token::Macro) => self.parse_macro_expression(),
            t => Err(Self::new_parse_error("prefix expression", t).into()),
        }
    }

    fn parse_infix(&mut self, left: ast::Expression) -> Result<(ast::Expression, bool)> {
        match self.peek_token() {
            Some(Token::Plus)
            | Some(Token::Minus)
            | Some(Token::Asterisk)
            | Some(Token::Slash)
            | Some(Token::LT)
            | Some(Token::GT)
            | Some(Token::Eq)
            | Some(Token::NotEq) => {
                self.next();
                let expr = self.parse_infix_expression(left)?;
                Ok((expr, true))
            }
            Some(Token::LParen) => {
                self.next();
                let expr = self.parse_call_expression(left)?;
                Ok((expr, true))
            }
            Some(Token::LBracket) => {
                self.next();
                let expr = self.parse_index_expression(left)?;
                Ok((expr, true))
            }
            _ => return Ok((left, false)),
        }
    }

    fn parse_identifier_expression(&mut self) -> Result<ast::Expression> {
        let ident = Self::parse_identifier(self.current_token())?;
        Ok(ident.into())
    }

    fn parse_integer_expression(&mut self) -> Result<ast::Expression> {
        match self.current_token() {
            Some(Token::Int(s)) => match s.parse::<i64>() {
                Ok(n) => Ok(ast::Expression::Integer(n)),
                Err(_) => Err(Self::new_parse_error("integer", self.current_token()).into()),
            },
            t => Err(Self::new_parse_error("integer", t).into()),
        }
    }

    fn parse_boolean_expression(&mut self) -> Result<ast::Expression> {
        match self.current_token() {
            Some(Token::True) => Ok(ast::Expression::Boolean(true)),
            Some(Token::False) => Ok(ast::Expression::Boolean(false)),
            t => Err(Self::new_parse_error("boolean", t).into()),
        }
    }

    fn parse_string_expression(&mut self) -> Result<ast::Expression> {
        match self.current_token() {
            Some(Token::String(s)) => Ok(ast::Expression::String(s.clone())),
            t => Err(Self::new_parse_error("string", t).into()),
        }
    }

    fn parse_array_expression(&mut self) -> Result<ast::Expression> {
        self.expect_current_token(Token::LBracket)?;
        let exprs = self.parse_expression_list(Token::RBracket)?;
        Ok(ast::Expression::Array(exprs))
    }

    fn parse_hash_expression(&mut self) -> Result<ast::Expression> {
        // {[<expr>: <expr>, ...]}
        self.expect_current_token(Token::LBrace)?;

        let is_not_end = |tok: Option<&Token>| tok.filter(|&t| t != &Token::RBrace).is_some();
        // <expr>: <expr>, [<expr>: <expr>, ...]
        let mut map = BTreeMap::new();
        while is_not_end(self.peek_token()) {
            self.next();
            let key = self.parse_expression(Precedence::Lowest)?;
            self.expect_peek_token_and_next(Token::Colon)?;
            self.next();
            let value = self.parse_expression(Precedence::Lowest)?;
            map.insert(key, value);
            if is_not_end(self.peek_token()) {
                self.expect_peek_token_and_next(Token::Comma)?;
            }
        }
        // }
        self.expect_peek_token_and_next(Token::RBrace)?;
        Ok(ast::Expression::Hash(map))
    }

    fn parse_prefix_expression(&mut self) -> Result<ast::Expression> {
        let operator = match self.current_token() {
            Some(Token::Bang) => ast::PrefixOperator::Bang,
            Some(Token::Minus) => ast::PrefixOperator::Minus,
            t => return Err(Self::new_parse_error("prefix operator", t).into()),
        };
        self.next();
        let right = self.parse_expression(Precedence::Prefix)?;
        Ok(ast::Expression::Prefix {
            operator,
            right: Box::new(right),
        })
    }

    fn parse_infix_expression(&mut self, left: ast::Expression) -> Result<ast::Expression> {
        let operator = match self.current_token() {
            Some(Token::Plus) => ast::InfixOperator::Add,
            Some(Token::Minus) => ast::InfixOperator::Sub,
            Some(Token::Asterisk) => ast::InfixOperator::Mul,
            Some(Token::Slash) => ast::InfixOperator::Div,
            Some(Token::LT) => ast::InfixOperator::LT,
            Some(Token::GT) => ast::InfixOperator::GT,
            Some(Token::Eq) => ast::InfixOperator::Eq,
            Some(Token::NotEq) => ast::InfixOperator::NotEq,
            t => return Err(Self::new_parse_error("infix operator", t).into()),
        };
        let precedence = self.current_prececence();
        self.next();
        let right = self.parse_expression(precedence)?;
        Ok(ast::Expression::Infix {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        })
    }

    fn parse_grouped_expression(&mut self) -> Result<ast::Expression> {
        // ( <expression> )
        self.expect_current_token(Token::LParen)?;
        self.next();
        let expr = self.parse_expression(Precedence::Lowest)?;
        self.expect_peek_token_and_next(Token::RParen)?;
        Ok(expr)
    }

    fn parse_if_expression(&mut self) -> Result<ast::Expression> {
        // if <condition> { <consequence> } [ else { <alternative> } ]

        // if
        self.expect_current_token(Token::If)?;
        // <condition>
        self.expect_peek_token_and_next(Token::LParen)?;
        self.next();
        let condition = self.parse_expression(Precedence::Lowest)?;
        self.expect_peek_token_and_next(Token::RParen)?;
        // { <consequence> }
        self.expect_peek_token_and_next(Token::LBrace)?;
        let consequence = self.parse_block_statement()?;
        // [ else { <alternative> } ]
        let alternative = if self.peek_token() == Some(&Token::Else) {
            self.next();
            self.expect_peek_token_and_next(Token::LBrace)?;
            Some(self.parse_block_statement()?)
        } else {
            None
        };
        Ok(ast::Expression::If {
            condition: Box::new(condition),
            consequence,
            alternative,
        })
    }

    fn parse_function_expression(&mut self) -> Result<ast::Expression> {
        // fn(<arguments>) { <body> }

        // fn(<arguments>)
        self.expect_current_token(Token::Function)?;
        self.expect_peek_token_and_next(Token::LParen)?;
        let params = self.parse_function_parameters()?;

        // { <body> }
        self.expect_peek_token_and_next(Token::LBrace)?;
        let body = self.parse_block_statement()?;

        Ok(ast::FunctionExpression { params, body }.into())
    }

    fn parse_call_expression(&mut self, function: ast::Expression) -> Result<ast::Expression> {
        let function: ast::CallExpressionFunction = match function {
            ast::Expression::Identifier(id) => id.into(),
            ast::Expression::Function(f) => f.into(),
            _ => {
                return Err(
                    format!("could not parse {:?} as call expression function", function).into(),
                )
            }
        };
        let args = self.parse_expression_list(Token::RParen)?;
        Ok(ast::Expression::Call { function, args })
    }

    fn parse_index_expression(&mut self, left: ast::Expression) -> Result<ast::Expression> {
        self.expect_current_token(Token::LBracket)?;
        self.next();
        let index = self.parse_expression(Precedence::Lowest)?;
        self.expect_peek_token_and_next(Token::RBracket)?;
        Ok(ast::Expression::Index {
            left: Box::new(left),
            index: Box::new(index),
        })
    }

    fn parse_quote_expression(&mut self) -> Result<ast::Expression> {
        // quote(<expr>)

        self.expect_current_token(Token::Quote)?;
        self.expect_peek_token_and_next(Token::LParen)?;
        self.next();
        let expr = self.parse_expression(Precedence::Lowest)?;
        self.expect_peek_token_and_next(Token::RParen)?;

        Ok(ast::Expression::Quote(expr.into()))
    }

    fn parse_unquote_expression(&mut self) -> Result<ast::Expression> {
        // unquote(<expr>)

        self.expect_current_token(Token::Unquote)?;
        self.expect_peek_token_and_next(Token::LParen)?;
        self.next();
        let expr = self.parse_expression(Precedence::Lowest)?;
        self.expect_peek_token_and_next(Token::RParen)?;

        Ok(ast::Expression::Unquote(expr.into()))
    }

    fn parse_macro_expression(&mut self) -> Result<ast::Expression> {
        // macro(<arguments>) { <body> }

        // macro(<arguments>)
        self.expect_current_token(Token::Macro)?;
        self.expect_peek_token_and_next(Token::LParen)?;
        let params = self.parse_macro_parameters()?;

        // { <body> }
        self.expect_peek_token_and_next(Token::LBrace)?;
        let body = self.parse_block_statement()?;

        Ok(ast::MacroExpression { params, body }.into())
    }

    fn parse_comma_separated_list<T, F: Fn(&mut Parser) -> Result<T>>(
        &mut self,
        end: Token,
        parser: F,
    ) -> Result<Vec<T>> {
        // [<any>, ...]<end>
        if self.peek_token() == Some(&end) {
            self.next();
            return Ok(Vec::new());
        }
        self.next();
        // <any>, [<any>, ...]
        let mut res = Vec::new();
        res.push(parser(self)?);
        while self.peek_token() == Some(&Token::Comma) {
            self.next();
            self.next();
            res.push(parser(self)?);
        }
        // <end>
        self.expect_peek_token_and_next(end)?;

        Ok(res)
    }

    fn parse_expression_list(&mut self, end: Token) -> Result<Vec<ast::Expression>> {
        // [<expr>, ...]<end>
        self.parse_comma_separated_list(end, |parser| parser.parse_expression(Precedence::Lowest))
    }

    fn parse_function_parameters(&mut self) -> Result<Vec<ast::Identifier>> {
        // ([<arg>, ...])
        self.expect_current_token(Token::LParen)?;
        self.parse_comma_separated_list(Token::RParen, |parser| {
            Self::parse_identifier(parser.current_token())
        })
    }

    fn parse_macro_parameters(&mut self) -> Result<Vec<ast::Identifier>> {
        // ([<arg>, ...])
        self.parse_function_parameters()
    }

    fn current_prececence(&self) -> Precedence {
        Self::token_precedence(self.current_token())
    }

    fn peek_prececence(&mut self) -> Precedence {
        Self::token_precedence(self.peek_token())
    }

    fn expect_current_token(&mut self, expected: Token) -> Result<()> {
        if self.current_token() == Some(&expected) {
            Ok(())
        } else {
            let s = format!("{:?}", expected);
            Err(Self::new_token_error(s, self.peek_token()).into())
        }
    }

    fn expect_peek_token(&mut self, expected: Token) -> Result<()> {
        if self.peek_token() == Some(&expected) {
            Ok(())
        } else {
            let s = format!("{:?}", expected);
            Err(Self::new_token_error(s, self.peek_token()).into())
        }
    }

    fn expect_peek_token_and_next(&mut self, expected: Token) -> Result<()> {
        self.expect_peek_token(expected)?;
        self.next();
        Ok(())
    }

    fn parse_identifier(token: Option<&Token>) -> Result<ast::Identifier> {
        match token {
            Some(Token::Identifier(id)) => Ok(id.into()),
            t => Err(Self::new_parse_error("identifier", t).into()),
        }
    }

    fn token_precedence(token: Option<&Token>) -> Precedence {
        match token {
            Some(Token::Plus) | Some(Token::Minus) => Precedence::Sum,
            Some(Token::Asterisk) | Some(Token::Slash) => Precedence::Product,
            Some(Token::LT) | Some(Token::GT) => Precedence::LessGreater,
            Some(Token::Eq) | Some(Token::NotEq) => Precedence::Equals,
            Some(Token::LParen) => Precedence::Call,
            Some(Token::LBracket) => Precedence::Index,
            _ => Precedence::Lowest,
        }
    }

    fn new_token_error(expected: impl Into<String>, actual: Option<&Token>) -> String {
        let actual = match actual {
            Some(t) => format!("{:?}", t),
            _ => "EOF".into(),
        };
        format!(
            "expected token to be {}, got {} instead",
            expected.into(),
            actual
        )
    }

    fn new_parse_error(expected: impl Into<String>, actual: Option<&Token>) -> String {
        let actual = match actual {
            Some(t) => format!("{:?}", t),
            _ => "EOF".into(),
        };
        format!("could not parse {} as {}", actual, expected.into())
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, Result};
    use crate::ast::{self};
    use crate::lexer::Lexer;

    #[test]
    fn parse_let_statement() -> Result<()> {
        // (input, identifer, value)
        let cases = vec![
            ("let x = 5;", "x", ast::Expression::Integer(5)),
            ("let y = true;", "y", ast::Expression::Boolean(true)),
            (
                "let foobar = y;",
                "foobar",
                ast::Expression::from(ast::Identifier("y".into())),
            ),
        ];
        for (input, id, value) in cases {
            let lexer = Lexer::new(input);
            let program = parse(lexer)?;
            assert_eq!(program.statements.len(), 1);
            let s = &program.statements[0];
            test_let_statement(s, id, value);
        }
        Ok(())
    }

    #[test]
    fn parse_return_statement() -> Result<()> {
        // (input, expression)
        let cases = vec![
            ("return 5;", ast::Expression::Integer(5)),
            ("return true;", ast::Expression::Boolean(true)),
            (
                "return 1 + foo;",
                ast::Expression::Infix {
                    left: Box::new(ast::Expression::Integer(1)),
                    operator: ast::InfixOperator::Add,
                    right: Box::new(ast::Expression::from(ast::Identifier("foo".into()))),
                },
            ),
        ];
        for (input, expression) in cases {
            let lexer = Lexer::new(input);
            let program = parse(lexer)?;
            assert_eq!(program.statements.len(), 1);
            let s = &program.statements[0];
            test_return_statement(s, expression);
        }
        Ok(())
    }

    #[test]
    fn parse_identifier_expression() -> Result<()> {
        let input = r#"
        foobar;
        "#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| test_identifier_expression(expr, "foobar"));
        Ok(())
    }

    #[test]
    fn parse_integer_expression() -> Result<()> {
        let input = r#"
        5;
        "#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| test_integer_expression(expr, 5));
        Ok(())
    }

    #[test]
    fn parse_string_expression() -> Result<()> {
        // (input, expected)
        let tests = vec![
            (r#""foobar""#, "foobar"),
            (r#""foo bar""#, "foo bar"),
            (r#""""#, ""),
        ];
        for (input, expected) in tests {
            let lexer = Lexer::new(input);
            let program = parse(lexer)?;
            assert_eq!(program.statements.len(), 1);
            let s = &program.statements[0];
            parse_expression_statement(s, |expr| match expr {
                ast::Expression::String(s) => assert_eq!(s, &expected),
                _ => panic!("expression is not string. got={:?}", expr),
            });
        }
        Ok(())
    }

    #[test]
    fn parse_array_expression() -> Result<()> {
        let input = r#"[1, 2 * 2, 3 + 3]"#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| match expr {
            ast::Expression::Array(v) => {
                assert_eq!(v.len(), 3);
                test_integer_expression(&v[0], 1);
                test_infix_expression(
                    &v[1],
                    ast::Expression::Integer(2),
                    ast::InfixOperator::Mul,
                    ast::Expression::Integer(2),
                );
                test_infix_expression(
                    &v[2],
                    ast::Expression::Integer(3),
                    ast::InfixOperator::Add,
                    ast::Expression::Integer(3),
                );
            }
            _ => panic!("expression is not array. got={:?}", expr),
        });
        Ok(())
    }

    #[test]
    fn parse_hash_string_key_expression() -> Result<()> {
        let input = r#"{"one": 1, "two": 2, "three": 3}"#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| match expr {
            ast::Expression::Hash(v) => {
                assert_eq!(v.len(), 3);
                test_integer_expression(&v[&new_string_expr("one")], 1);
                test_integer_expression(&v[&new_string_expr("two")], 2);
                test_integer_expression(&v[&new_string_expr("three")], 3);
            }
            _ => panic!("expression is not hash. got={:?}", expr),
        });
        Ok(())
    }

    #[test]
    fn parse_empty_hash_expression() -> Result<()> {
        let input = r#"{}"#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| match expr {
            ast::Expression::Hash(v) => {
                assert!(v.is_empty());
            }
            _ => panic!("expression is not hash. got={:?}", expr),
        });
        Ok(())
    }
    #[test]
    fn parse_hash_expr_value_expression() -> Result<()> {
        let input = r#"{"one": 0 + 1, "two": 10 - 8, "three": 15 / 5}"#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| match expr {
            ast::Expression::Hash(v) => {
                assert_eq!(v.len(), 3);
                test_infix_expression(
                    &v[&new_string_expr("one")],
                    new_int_expr(0),
                    ast::InfixOperator::Add,
                    new_int_expr(1),
                );
                test_infix_expression(
                    &v[&new_string_expr("two")],
                    new_int_expr(10),
                    ast::InfixOperator::Sub,
                    new_int_expr(8),
                );
                test_infix_expression(
                    &v[&new_string_expr("three")],
                    new_int_expr(15),
                    ast::InfixOperator::Div,
                    new_int_expr(5),
                );
            }
            _ => panic!("expression is not hash. got={:?}", expr),
        });
        Ok(())
    }

    #[test]
    fn parse_prefix_expression() -> Result<()> {
        // (input, operator, right)
        let cases = vec![
            (
                "!5;",
                ast::PrefixOperator::Bang,
                ast::Expression::Integer(5),
            ),
            (
                "-15;",
                ast::PrefixOperator::Minus,
                ast::Expression::Integer(15),
            ),
            (
                "!true;",
                ast::PrefixOperator::Bang,
                ast::Expression::Boolean(true),
            ),
            (
                "!false;",
                ast::PrefixOperator::Bang,
                ast::Expression::Boolean(false),
            ),
        ];
        for (input, op, r) in cases {
            let lexer = Lexer::new(input);
            let program = parse(lexer)?;
            assert_eq!(program.statements.len(), 1);
            let s = &program.statements[0];
            parse_expression_statement(s, |expr| match expr {
                ast::Expression::Prefix { .. } => {
                    test_prefix_expression(expr, op, r);
                }
                _ => panic!("expression not prefix. got={:?}", expr),
            });
        }
        Ok(())
    }

    #[test]
    fn parse_infix_expression() -> Result<()> {
        // (input, left, operator, right)
        let cases = vec![
            (
                "5 + 5;",
                ast::Expression::Integer(5),
                ast::InfixOperator::Add,
                ast::Expression::Integer(5),
            ),
            (
                "5 - 5;",
                ast::Expression::Integer(5),
                ast::InfixOperator::Sub,
                ast::Expression::Integer(5),
            ),
            (
                "5 * 5;",
                ast::Expression::Integer(5),
                ast::InfixOperator::Mul,
                ast::Expression::Integer(5),
            ),
            (
                "5 / 5;",
                ast::Expression::Integer(5),
                ast::InfixOperator::Div,
                ast::Expression::Integer(5),
            ),
            (
                "5 > 5;",
                ast::Expression::Integer(5),
                ast::InfixOperator::GT,
                ast::Expression::Integer(5),
            ),
            (
                "5 < 5;",
                ast::Expression::Integer(5),
                ast::InfixOperator::LT,
                ast::Expression::Integer(5),
            ),
            (
                "5 == 5;",
                ast::Expression::Integer(5),
                ast::InfixOperator::Eq,
                ast::Expression::Integer(5),
            ),
            (
                "5 != 5;",
                ast::Expression::Integer(5),
                ast::InfixOperator::NotEq,
                ast::Expression::Integer(5),
            ),
            (
                "true == true;",
                ast::Expression::Boolean(true),
                ast::InfixOperator::Eq,
                ast::Expression::Boolean(true),
            ),
            (
                "true != false;",
                ast::Expression::Boolean(true),
                ast::InfixOperator::NotEq,
                ast::Expression::Boolean(false),
            ),
            (
                "false == false;",
                ast::Expression::Boolean(false),
                ast::InfixOperator::Eq,
                ast::Expression::Boolean(false),
            ),
        ];
        for (input, l, op, r) in cases {
            let lexer = Lexer::new(input);
            let program = parse(lexer)?;
            assert_eq!(program.statements.len(), 1);
            let s = &program.statements[0];
            parse_expression_statement(s, |expr| match expr {
                ast::Expression::Infix { .. } => {
                    test_infix_expression(expr, l, op, r);
                }
                _ => panic!("expression not infix. got={:?}", expr),
            });
        }
        Ok(())
    }

    #[test]
    fn parse_if_expression() -> Result<()> {
        let input = r#"
        if (x < y) { x }
        "#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| match expr {
            ast::Expression::If {
                condition,
                consequence,
                ..
            } => {
                // condition
                test_infix_expression(
                    condition,
                    ast::Expression::from(ast::Identifier("x".into())),
                    ast::InfixOperator::LT,
                    ast::Expression::from(ast::Identifier("y".into())),
                );
                // consequence
                assert_eq!(consequence.statements.len(), 1);
                let s = &consequence.statements[0];
                match s {
                    ast::Statement::Expression(expr) => test_identifier_expression(expr, "x"),
                    _ => panic!("statement not `<expr>`. got={:?}", s),
                };
            }
            _ => panic!("expression not if. got={:?}", expr),
        });
        Ok(())
    }

    #[test]
    fn parse_if_else_expression() -> Result<()> {
        let input = r#"
        if (x < y) { x } else { y }
        "#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| match expr {
            ast::Expression::If {
                condition,
                consequence,
                alternative,
            } => {
                // condition
                test_infix_expression(
                    condition,
                    ast::Expression::from(ast::Identifier("x".into())),
                    ast::InfixOperator::LT,
                    ast::Expression::from(ast::Identifier("y".into())),
                );
                // consequence
                assert_eq!(consequence.statements.len(), 1);
                let s = &consequence.statements[0];
                match s {
                    ast::Statement::Expression(expr) => test_identifier_expression(expr, "x"),
                    _ => panic!("statement not `<expr>`. got={:?}", s),
                };
                // alternative
                assert!(alternative.is_some());
                let alternative = alternative.as_ref().unwrap();
                assert_eq!(alternative.statements.len(), 1);
                let s = &alternative.statements[0];
                match s {
                    ast::Statement::Expression(expr) => test_identifier_expression(expr, "y"),
                    _ => panic!("statement not `<expr>`. got={:?}", s),
                };
            }
            _ => panic!("expression not if. got={:?}", expr),
        });
        Ok(())
    }

    #[test]
    fn parse_function_expression() -> Result<()> {
        let input = r#"
        fn(x, y) { x + y; }
        "#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| match expr {
            ast::Expression::Function(ast::FunctionExpression { params, body }) => {
                // parameters
                assert_eq!(params.len(), 2);
                test_identifier(&params[0], "x");
                test_identifier(&params[1], "y");
                // body
                assert_eq!(body.statements.len(), 1);
                let s = &body.statements[0];
                match s {
                    ast::Statement::Expression(expr) => test_infix_expression(
                        expr,
                        ast::Expression::Identifier(ast::Identifier("x".into())),
                        ast::InfixOperator::Add,
                        ast::Expression::Identifier(ast::Identifier("y".into())),
                    ),
                    _ => panic!("statement not `<expr>`. got={:?}", s),
                };
            }
            _ => panic!("expression not function. got={:?}", expr),
        });
        Ok(())
    }

    #[test]
    fn parse_call_expression() -> Result<()> {
        let input = r#"
        add(1, 2 * 3, 4 + 5);
        "#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| match expr {
            ast::Expression::Call { function, args } => {
                // function
                match function {
                    ast::CallExpressionFunction::Identifier(id) => test_identifier(id, "add"),
                    _ => panic!("function is not identifier. got={:?}", function),
                }
                // args
                assert_eq!(args.len(), 3);
                test_integer_expression(&args[0], 1);
                test_infix_expression(
                    &args[1],
                    ast::Expression::Integer(2),
                    ast::InfixOperator::Mul,
                    ast::Expression::Integer(3),
                );
                test_infix_expression(
                    &args[2],
                    ast::Expression::Integer(4),
                    ast::InfixOperator::Add,
                    ast::Expression::Integer(5),
                );
            }
            _ => panic!("expression not function. got={:?}", expr),
        });
        Ok(())
    }

    #[test]
    fn parse_index_expression() -> Result<()> {
        let input = r#"myArray[1 + 1]"#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| match expr {
            ast::Expression::Index { left, index } => {
                test_identifier_expression(left, "myArray");
                test_infix_expression(
                    index,
                    ast::Expression::Integer(1),
                    ast::InfixOperator::Add,
                    ast::Expression::Integer(1),
                );
            }
            _ => panic!("expression is not array. got={:?}", expr),
        });
        Ok(())
    }

    #[test]
    fn parse_macro_expression() -> Result<()> {
        let input = r#"
        macro(x, y) { x + y; }
        "#;
        let lexer = Lexer::new(input);
        let program = parse(lexer)?;
        assert_eq!(program.statements.len(), 1);
        let s = &program.statements[0];
        parse_expression_statement(s, |expr| match expr {
            ast::Expression::Macro(m) => {
                let ast::MacroExpression { params, body } = m;
                // parameters
                assert_eq!(params.len(), 2);
                test_identifier(&params[0], "x");
                test_identifier(&params[1], "y");
                // body
                assert_eq!(body.statements.len(), 1);
                let s = &body.statements[0];
                match s {
                    ast::Statement::Expression(expr) => test_infix_expression(
                        expr,
                        ast::Expression::Identifier(ast::Identifier("x".into())),
                        ast::InfixOperator::Add,
                        ast::Expression::Identifier(ast::Identifier("y".into())),
                    ),
                    _ => panic!("statement not `<expr>`. got={:?}", s),
                };
            }
            _ => panic!("expression not macro. got={:?}", expr),
        });
        Ok(())
    }

    #[test]
    fn parse_precedence() -> Result<()> {
        // (input, expected)
        let cases = vec![
            ("-a * b", "((-a) * b)"),
            ("!-a", "(!(-a))"),
            ("a + b + c", "((a + b) + c)"),
            ("a + b - c", "((a + b) - c)"),
            ("a * b * c", "((a * b) * c)"),
            ("a * b / c", "((a * b) / c)"),
            ("a + b / c", "(a + (b / c))"),
            ("a + b * c + d / e - f", "(((a + (b * c)) + (d / e)) - f)"),
            ("3 + 4; -5 * 5", "(3 + 4)((-5) * 5)"),
            ("5 > 4 == 3 < 4", "((5 > 4) == (3 < 4))"),
            ("5 > 4 != 3 < 4", "((5 > 4) != (3 < 4))"),
            (
                "3 + 4 * 5 == 3 * 1 + 4 * 5",
                "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)))",
            ),
            ("true", "true"),
            ("false", "false"),
            ("3 > 5 == false", "((3 > 5) == false)"),
            ("3 < 5 == true", "((3 < 5) == true)"),
            ("1 + (2 + 3) + 4", "((1 + (2 + 3)) + 4)"),
            ("(5 + 5) * 2", "((5 + 5) * 2)"),
            ("2 / (5 + 5)", "(2 / (5 + 5))"),
            ("-(5 + 5)", "(-(5 + 5))"),
            ("!(true == true)", "(!(true == true))"),
            ("a + add(b * c) + d", "((a + add((b * c))) + d)"),
            (
                "add(a, b, 1, 2 * 3, 4 + 5, add(6, 7 * 8))",
                "add(a, b, 1, (2 * 3), (4 + 5), add(6, (7 * 8)))",
            ),
            (
                "add(a + b + c * d / f + g)",
                "add((((a + b) + ((c * d) / f)) + g))",
            ),
            (
                "a * [1, 2, 3, 4][b * c] * d",
                "((a * ([1, 2, 3, 4][(b * c)])) * d)",
            ),
            (
                "add(a * b[2], b[1], 2 * [1, 2][1])",
                "add((a * (b[2])), (b[1]), (2 * ([1, 2][1])))",
            ),
        ];
        for (input, expected) in cases {
            let lexer = Lexer::new(input);
            let program = parse(lexer)?;
            let s = format!("{}", program);
            assert_eq!(s, expected);
        }
        Ok(())
    }

    #[test]
    fn display() {
        let program = ast::Program {
            statements: vec![ast::Statement::Let {
                identifier: ast::Identifier("my_var".into()),
                expression: ast::Identifier("another_var".into()).into(),
            }],
        };
        let source = format!("{}", program);
        assert_eq!(source, "let my_var = another_var;".to_string());
    }

    // heplers

    fn parse_expression_statement<F>(s: &ast::Statement, f: F)
    where
        F: FnOnce(&ast::Expression),
    {
        match s {
            ast::Statement::Expression(expr) => f(expr),
            _ => panic!("statement is not `Expression`. got={:?}", s),
        };
    }

    fn test_let_statement(s: &ast::Statement, id: impl Into<String>, value: ast::Expression) {
        match s {
            ast::Statement::Let {
                identifier,
                expression,
            } => {
                test_identifier(identifier, id);
                assert_eq!(expression, &value);
            }
            _ => panic!("statement not `let`. got={:?}", s),
        };
    }

    fn test_return_statement(s: &ast::Statement, expr: ast::Expression) {
        match s {
            ast::Statement::Return(e) => assert_eq!(e, &expr),
            _ => panic!("statement not `return`. got={:?}", s),
        };
    }

    fn test_identifier_expression(expr: &ast::Expression, name: impl Into<String>) {
        match expr {
            ast::Expression::Identifier(id) => {
                test_identifier(id, name);
            }
            _ => panic!("expression not identifier. got={:?}", expr),
        };
    }

    fn test_identifier(id: &ast::Identifier, name: impl Into<String>) {
        let s = id.0.to_string();
        let name = name.into();
        assert_eq!(s, name, "identifier not {}. got={}", s, name);
    }

    fn test_integer_expression(expr: &ast::Expression, num: i64) {
        match expr {
            ast::Expression::Integer(n) => {
                assert_eq!(*n, num, "integer not {}. got={}", num, n);
            }
            _ => panic!("expression not Integer. got={:?}", expr),
        }
    }

    fn test_prefix_expression(
        expr: &ast::Expression,
        operator: ast::PrefixOperator,
        right: ast::Expression,
    ) {
        match expr {
            ast::Expression::Prefix {
                operator: op,
                right: r,
            } => {
                assert_eq!(op, &operator);
                assert_eq!(r, &Box::new(right));
            }
            _ => panic!("expression not prefix. got={:?}", expr),
        }
    }

    fn test_infix_expression(
        expr: &ast::Expression,
        left: ast::Expression,
        operator: ast::InfixOperator,
        right: ast::Expression,
    ) {
        match expr {
            ast::Expression::Infix {
                left: l,
                operator: op,
                right: r,
            } => {
                assert_eq!(l, &Box::new(left));
                assert_eq!(op, &operator);
                assert_eq!(r, &Box::new(right));
            }
            _ => panic!("expression not infix. got={:?}", expr),
        }
    }

    fn new_int_expr(n: i64) -> ast::Expression {
        ast::Expression::Integer(n)
    }

    fn new_string_expr(s: impl Into<String>) -> ast::Expression {
        ast::Expression::String(s.into())
    }
}
