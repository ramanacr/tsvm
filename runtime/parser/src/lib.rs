#![forbid(unsafe_code)]

use std::{fmt, mem::discriminant};

use tsvm_ast::{
    AssignmentOperator, BinaryOperator, ClassDeclaration, ClassMember, Expression, ExpressionKind,
    ForStatement, FunctionDeclaration, IfStatement, ImportDeclaration, InterfaceDeclaration,
    ObjectProperty, Parameter, Program, Span, Statement, StatementKind, TypeAliasDeclaration,
    TypeKind, TypeMember, TypeNode, UnaryOperator, VariableDeclaration, VariableKind,
    WhileStatement,
};
use tsvm_lexer::{lex, LexError, Position, Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseOutput {
    pub program: Program,
    pub diagnostics: Vec<ParseDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for ParseDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {}:{}",
            self.message, self.span.start.line, self.span.start.column
        )
    }
}

pub fn parse_source(source: &str) -> ParseOutput {
    match lex(source) {
        Ok(tokens) => Parser::new(tokens).parse_program(),
        Err(err) => ParseOutput {
            program: Program {
                body: Vec::new(),
                span: None,
            },
            diagnostics: vec![diagnostic_from_lex_error(err)],
        },
    }
}

fn diagnostic_from_lex_error(err: LexError) -> ParseDiagnostic {
    ParseDiagnostic {
        message: err.to_string(),
        span: err.span,
    }
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    diagnostics: Vec<ParseDiagnostic>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            cursor: 0,
            diagnostics: Vec::new(),
        }
    }

    fn parse_program(mut self) -> ParseOutput {
        let mut body = Vec::new();

        while !self.is_at_end() {
            let start_cursor = self.cursor;
            body.push(self.parse_statement());
            if self.cursor == start_cursor {
                self.error_here("parser made no progress");
                self.advance();
            }
        }

        let span = merge_statement_spans(&body);
        ParseOutput {
            program: Program { body, span },
            diagnostics: self.diagnostics,
        }
    }

    fn parse_statement(&mut self) -> Statement {
        match self.peek_kind() {
            Some(TokenKind::Import) => self.parse_import(),
            Some(TokenKind::Export) => self.parse_export(),
            Some(TokenKind::Interface) => self.parse_interface(),
            Some(TokenKind::Type) => self.parse_type_alias(),
            Some(TokenKind::Class) => self.parse_class(),
            Some(TokenKind::Function) => self.parse_function(),
            Some(TokenKind::Let | TokenKind::Const | TokenKind::Var) => self.parse_variable(true),
            Some(TokenKind::Return) => self.parse_return(),
            Some(TokenKind::Throw) => self.parse_throw(),
            Some(TokenKind::If) => self.parse_if(),
            Some(TokenKind::While) => self.parse_while(),
            Some(TokenKind::For) => self.parse_for(),
            Some(TokenKind::LeftBrace) => self.parse_block_statement(),
            Some(TokenKind::Semicolon) => {
                let span = self.advance().expect("semicolon exists").span;
                Statement {
                    kind: StatementKind::Empty,
                    span,
                }
            }
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_import(&mut self) -> Statement {
        let start = self.expect_any("import declaration start").span.start;
        let mut names = Vec::new();

        if self.match_kind(&TokenKind::LeftBrace) {
            while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
                if let Some(name) = self.consume_identifier("import binding") {
                    names.push(name);
                }
                if !self.match_kind(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RightBrace, "closing import brace");
        } else if let Some(name) = self.consume_identifier("default import binding") {
            names.push(name);
        }

        self.expect(&TokenKind::From, "`from` in import declaration");
        let from = match self.advance() {
            Some(Token {
                kind: TokenKind::StringLiteral(value),
                ..
            }) => value,
            Some(token) => {
                self.error_at(token.span, "expected string module specifier");
                String::new()
            }
            None => {
                self.error_here("expected string module specifier");
                String::new()
            }
        };
        let end = self.consume_optional_semicolon();

        Statement {
            kind: StatementKind::Import(ImportDeclaration { names, from }),
            span: Span {
                start,
                end: end.unwrap_or_else(|| self.previous_end(start)),
            },
        }
    }

    fn parse_export(&mut self) -> Statement {
        let start = self.expect_any("export declaration start").span.start;
        let inner = self.parse_statement();
        let span = Span {
            start,
            end: inner.span.end,
        };
        Statement {
            kind: StatementKind::Export(Box::new(inner)),
            span,
        }
    }

    fn parse_interface(&mut self) -> Statement {
        let start = self.expect_any("interface declaration start").span.start;
        let name = self
            .consume_identifier("interface name")
            .unwrap_or_else(|| "<error>".into());
        let members = self.parse_type_members();
        let end = self.previous_end(start);

        Statement {
            kind: StatementKind::Interface(InterfaceDeclaration { name, members }),
            span: Span { start, end },
        }
    }

    fn parse_type_alias(&mut self) -> Statement {
        let start = self.expect_any("type alias start").span.start;
        let name = self
            .consume_identifier("type alias name")
            .unwrap_or_else(|| "<error>".into());
        self.expect(&TokenKind::Equals, "`=` in type alias");
        let ty = self.parse_type();
        let end = self
            .consume_optional_semicolon()
            .unwrap_or_else(|| self.previous_end(start));

        Statement {
            kind: StatementKind::TypeAlias(TypeAliasDeclaration { name, ty }),
            span: Span { start, end },
        }
    }

    fn parse_class(&mut self) -> Statement {
        let start = self.expect_any("class declaration start").span.start;
        let name = self
            .consume_identifier("class name")
            .unwrap_or_else(|| "<error>".into());
        let mut members = Vec::new();

        self.expect(&TokenKind::LeftBrace, "class body");
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let member_start = self.current_start();
            let Some(name) = self.consume_identifier("class member name") else {
                self.synchronize_member();
                continue;
            };
            let ty = if self.match_kind(&TokenKind::Colon) {
                Some(self.parse_type())
            } else {
                None
            };
            let initializer = if self.match_kind(&TokenKind::Equals) {
                Some(self.parse_expression())
            } else {
                None
            };
            let end = self
                .consume_optional_semicolon()
                .unwrap_or_else(|| self.previous_end(member_start));
            members.push(ClassMember {
                name,
                ty,
                initializer,
                span: Span {
                    start: member_start,
                    end,
                },
            });
        }
        self.expect(&TokenKind::RightBrace, "closing class body");
        let end = self.previous_end(start);

        Statement {
            kind: StatementKind::Class(ClassDeclaration { name, members }),
            span: Span { start, end },
        }
    }

    fn parse_function(&mut self) -> Statement {
        let start = self.expect_any("function declaration start").span.start;
        let name = self
            .consume_identifier("function name")
            .unwrap_or_else(|| "<error>".into());
        let params = self.parse_parameters();
        let return_type = if self.match_kind(&TokenKind::Colon) {
            Some(self.parse_type())
        } else {
            None
        };
        let body_statement = self.parse_block_statement();
        let (body, end) = match body_statement.kind {
            StatementKind::Block(body) => (body, body_statement.span.end),
            _ => (Vec::new(), body_statement.span.end),
        };

        Statement {
            kind: StatementKind::Function(FunctionDeclaration {
                name,
                params,
                return_type,
                body,
            }),
            span: Span { start, end },
        }
    }

    fn parse_parameters(&mut self) -> Vec<Parameter> {
        let mut params = Vec::new();
        self.expect(&TokenKind::LeftParen, "parameter list");

        while !self.check(&TokenKind::RightParen) && !self.is_at_end() {
            let start = self.current_start();
            let name = self
                .consume_identifier("parameter name")
                .unwrap_or_else(|| "<error>".into());
            let ty = if self.match_kind(&TokenKind::Colon) {
                Some(self.parse_type())
            } else {
                None
            };
            params.push(Parameter {
                name,
                ty,
                span: Span {
                    start,
                    end: self.previous_end(start),
                },
            });
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }

        self.expect(&TokenKind::RightParen, "closing parameter list");
        params
    }

    fn parse_variable(&mut self, consume_semicolon: bool) -> Statement {
        let start_token = self.expect_any("variable declaration start");
        let start = start_token.span.start;
        let kind = match start_token.kind {
            TokenKind::Let => VariableKind::Let,
            TokenKind::Const => VariableKind::Const,
            TokenKind::Var => VariableKind::Var,
            _ => unreachable!("parse_variable starts with variable keyword"),
        };
        let name = self
            .consume_identifier("variable name")
            .unwrap_or_else(|| "<error>".into());
        let ty = if self.match_kind(&TokenKind::Colon) {
            Some(self.parse_type())
        } else {
            None
        };
        let initializer = if self.match_kind(&TokenKind::Equals) {
            Some(self.parse_expression())
        } else {
            None
        };
        let end = if consume_semicolon {
            self.consume_optional_semicolon()
                .unwrap_or_else(|| self.previous_end(start))
        } else {
            self.previous_end(start)
        };

        Statement {
            kind: StatementKind::Variable(VariableDeclaration {
                kind,
                name,
                ty,
                initializer,
            }),
            span: Span { start, end },
        }
    }

    fn parse_return(&mut self) -> Statement {
        let start = self.expect_any("return statement start").span.start;
        let value = if self.check(&TokenKind::Semicolon) || self.check(&TokenKind::RightBrace) {
            None
        } else {
            Some(self.parse_expression())
        };
        let end = self
            .consume_optional_semicolon()
            .unwrap_or_else(|| self.previous_end(start));

        Statement {
            kind: StatementKind::Return(value),
            span: Span { start, end },
        }
    }

    fn parse_throw(&mut self) -> Statement {
        let start = self.expect_any("throw statement start").span.start;
        let value = self.parse_expression();
        let end = self
            .consume_optional_semicolon()
            .unwrap_or_else(|| self.previous_end(start));

        Statement {
            kind: StatementKind::Throw(value),
            span: Span { start, end },
        }
    }

    fn parse_if(&mut self) -> Statement {
        let start = self.expect_any("if statement start").span.start;
        self.expect(&TokenKind::LeftParen, "if condition");
        let test = self.parse_expression();
        self.expect(&TokenKind::RightParen, "closing if condition");
        let consequent = Box::new(self.parse_statement());
        let alternate = if self.match_kind(&TokenKind::Else) {
            Some(Box::new(self.parse_statement()))
        } else {
            None
        };
        let end = alternate
            .as_ref()
            .map_or(consequent.span.end, |statement| statement.span.end);

        Statement {
            kind: StatementKind::If(IfStatement {
                test,
                consequent,
                alternate,
            }),
            span: Span { start, end },
        }
    }

    fn parse_while(&mut self) -> Statement {
        let start = self.expect_any("while statement start").span.start;
        self.expect(&TokenKind::LeftParen, "while condition");
        let test = self.parse_expression();
        self.expect(&TokenKind::RightParen, "closing while condition");
        let body = Box::new(self.parse_statement());
        let end = body.span.end;

        Statement {
            kind: StatementKind::While(WhileStatement { test, body }),
            span: Span { start, end },
        }
    }

    fn parse_for(&mut self) -> Statement {
        let start = self.expect_any("for statement start").span.start;
        self.expect(&TokenKind::LeftParen, "for header");

        let initializer = if self.match_kind(&TokenKind::Semicolon) {
            None
        } else if matches!(
            self.peek_kind(),
            Some(TokenKind::Let | TokenKind::Const | TokenKind::Var)
        ) {
            Some(Box::new(self.parse_variable(false)))
        } else {
            Some(Box::new(
                self.parse_expression_statement_without_semicolon(),
            ))
        };
        self.expect(&TokenKind::Semicolon, "semicolon after for initializer");

        let test = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression())
        };
        self.expect(&TokenKind::Semicolon, "semicolon after for test");

        let update = if self.check(&TokenKind::RightParen) {
            None
        } else {
            Some(self.parse_expression())
        };
        self.expect(&TokenKind::RightParen, "closing for header");
        let body = Box::new(self.parse_statement());
        let end = body.span.end;

        Statement {
            kind: StatementKind::For(ForStatement {
                initializer,
                test,
                update,
                body,
            }),
            span: Span { start, end },
        }
    }

    fn parse_block_statement(&mut self) -> Statement {
        let start = self.current_start();
        self.expect(&TokenKind::LeftBrace, "block start");
        let mut body = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            body.push(self.parse_statement());
        }

        self.expect(&TokenKind::RightBrace, "block end");
        let end = self.previous_end(start);
        Statement {
            kind: StatementKind::Block(body),
            span: Span { start, end },
        }
    }

    fn parse_expression_statement(&mut self) -> Statement {
        let statement = self.parse_expression_statement_without_semicolon();
        self.consume_optional_semicolon();
        statement
    }

    fn parse_expression_statement_without_semicolon(&mut self) -> Statement {
        let expression = self.parse_expression();
        let span = expression.span;
        Statement {
            kind: StatementKind::Expression(expression),
            span,
        }
    }

    fn parse_expression(&mut self) -> Expression {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Expression {
        let left = self.parse_binary(0);
        let Some(operator) = self.assignment_operator() else {
            return left;
        };
        self.advance();
        let right = self.parse_assignment();
        let span = Span {
            start: left.span.start,
            end: right.span.end,
        };
        Expression {
            kind: ExpressionKind::Assignment {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            },
            span,
        }
    }

    fn parse_binary(&mut self, min_precedence: u8) -> Expression {
        let mut left = self.parse_unary();

        while let Some((operator, precedence)) = self.binary_operator() {
            if precedence < min_precedence {
                break;
            }

            self.advance();
            let right = self.parse_binary(precedence + 1);
            let span = Span {
                start: left.span.start,
                end: right.span.end,
            };
            left = Expression {
                kind: ExpressionKind::Binary {
                    left: Box::new(left),
                    operator,
                    right: Box::new(right),
                },
                span,
            };
        }

        left
    }

    fn parse_unary(&mut self) -> Expression {
        let Some(token) = self.peek().cloned() else {
            return self.error_expression();
        };

        let operator = match token.kind {
            TokenKind::Bang => Some(UnaryOperator::Not),
            TokenKind::Minus => Some(UnaryOperator::Negate),
            _ => None,
        };

        if let Some(operator) = operator {
            self.advance();
            let argument = self.parse_unary();
            let span = Span {
                start: token.span.start,
                end: argument.span.end,
            };
            Expression {
                kind: ExpressionKind::Unary {
                    operator,
                    argument: Box::new(argument),
                },
                span,
            }
        } else {
            self.parse_call_member()
        }
    }

    fn parse_call_member(&mut self) -> Expression {
        let mut expression = self.parse_primary();

        loop {
            if self.match_kind(&TokenKind::Dot) {
                let start = expression.span.start;
                let property = self
                    .consume_identifier("member property name")
                    .unwrap_or_else(|| "<error>".into());
                let end = self.previous_end(expression.span.start);
                expression = Expression {
                    kind: ExpressionKind::Member {
                        object: Box::new(expression),
                        property,
                    },
                    span: Span { start, end },
                };
            } else if self.match_kind(&TokenKind::LeftParen) {
                let start = expression.span.start;
                let mut arguments = Vec::new();
                while !self.check(&TokenKind::RightParen) && !self.is_at_end() {
                    arguments.push(self.parse_expression());
                    if !self.match_kind(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(&TokenKind::RightParen, "closing call arguments");
                expression = Expression {
                    kind: ExpressionKind::Call {
                        callee: Box::new(expression),
                        arguments,
                    },
                    span: Span {
                        start,
                        end: self.previous_end(start),
                    },
                };
            } else {
                break;
            }
        }

        expression
    }

    fn parse_primary(&mut self) -> Expression {
        let Some(token) = self.advance() else {
            return self.error_expression();
        };
        let span = token.span;

        match token.kind {
            TokenKind::Identifier(name) => Expression {
                kind: ExpressionKind::Identifier(name),
                span,
            },
            TokenKind::NumberLiteral(value) => Expression {
                kind: ExpressionKind::Number(value),
                span,
            },
            TokenKind::StringLiteral(value) => Expression {
                kind: ExpressionKind::String(value),
                span,
            },
            TokenKind::BooleanLiteral(value) => Expression {
                kind: ExpressionKind::Boolean(value),
                span,
            },
            TokenKind::Null => Expression {
                kind: ExpressionKind::Null,
                span,
            },
            TokenKind::Undefined => Expression {
                kind: ExpressionKind::Undefined,
                span,
            },
            TokenKind::LeftParen => {
                let expression = self.parse_expression();
                self.expect(&TokenKind::RightParen, "closing parenthesized expression");
                expression
            }
            TokenKind::LeftBrace => self.finish_object_expression(span.start),
            TokenKind::LeftBracket => self.finish_array_expression(span.start),
            unexpected => {
                self.error_at(span, &format!("expected expression, found {unexpected:?}"));
                Expression {
                    kind: ExpressionKind::Error,
                    span,
                }
            }
        }
    }

    fn finish_object_expression(&mut self, start: Position) -> Expression {
        let mut properties = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let property_start = self.current_start();
            let name = self
                .consume_property_name("object property name")
                .unwrap_or_else(|| "<error>".into());
            self.expect(&TokenKind::Colon, "colon after object property name");
            let value = self.parse_expression();
            let span = Span {
                start: property_start,
                end: value.span.end,
            };
            properties.push(ObjectProperty { name, value, span });
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }

        self.expect(&TokenKind::RightBrace, "closing object literal");
        Expression {
            kind: ExpressionKind::Object(properties),
            span: Span {
                start,
                end: self.previous_end(start),
            },
        }
    }

    fn finish_array_expression(&mut self, start: Position) -> Expression {
        let mut elements = Vec::new();

        while !self.check(&TokenKind::RightBracket) && !self.is_at_end() {
            elements.push(self.parse_expression());
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }

        self.expect(&TokenKind::RightBracket, "closing array literal");
        Expression {
            kind: ExpressionKind::Array(elements),
            span: Span {
                start,
                end: self.previous_end(start),
            },
        }
    }

    fn parse_type_members(&mut self) -> Vec<TypeMember> {
        let mut members = Vec::new();
        self.expect(&TokenKind::LeftBrace, "type member list");

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let start = self.current_start();
            let name = self
                .consume_property_name("type member name")
                .unwrap_or_else(|| "<error>".into());
            self.expect(&TokenKind::Colon, "colon after type member name");
            let ty = self.parse_type();
            let end = self
                .consume_optional_semicolon()
                .or_else(|| {
                    if self.match_kind(&TokenKind::Comma) {
                        Some(self.previous_end(start))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| self.previous_end(start));
            members.push(TypeMember {
                name,
                ty,
                span: Span { start, end },
            });
        }

        self.expect(&TokenKind::RightBrace, "closing type member list");
        members
    }

    fn parse_type(&mut self) -> TypeNode {
        let start = self.current_start();
        let Some(token) = self.advance() else {
            self.error_here("expected type");
            return self.error_type(start);
        };

        let mut ty = match token.kind {
            TokenKind::NumberType => TypeNode {
                kind: TypeKind::Number,
                span: token.span,
            },
            TokenKind::StringType => TypeNode {
                kind: TypeKind::String,
                span: token.span,
            },
            TokenKind::BooleanType => TypeNode {
                kind: TypeKind::Boolean,
                span: token.span,
            },
            TokenKind::Null => TypeNode {
                kind: TypeKind::Null,
                span: token.span,
            },
            TokenKind::Undefined => TypeNode {
                kind: TypeKind::Undefined,
                span: token.span,
            },
            TokenKind::Identifier(name) => TypeNode {
                kind: TypeKind::Named(name),
                span: token.span,
            },
            TokenKind::LeftBrace => {
                let members = self.finish_type_members_after_left_brace();
                TypeNode {
                    kind: TypeKind::Object(members),
                    span: Span {
                        start,
                        end: self.previous_end(start),
                    },
                }
            }
            unexpected => {
                self.error_at(token.span, &format!("expected type, found {unexpected:?}"));
                self.error_type(token.span.start)
            }
        };

        while self.match_kind(&TokenKind::LeftBracket) {
            self.expect(&TokenKind::RightBracket, "closing array type suffix");
            ty = TypeNode {
                kind: TypeKind::Array(Box::new(ty)),
                span: Span {
                    start,
                    end: self.previous_end(start),
                },
            };
        }

        ty
    }

    fn finish_type_members_after_left_brace(&mut self) -> Vec<TypeMember> {
        let mut members = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let start = self.current_start();
            let name = self
                .consume_property_name("type member name")
                .unwrap_or_else(|| "<error>".into());
            self.expect(&TokenKind::Colon, "colon after type member name");
            let ty = self.parse_type();
            let end = self
                .consume_optional_semicolon()
                .or_else(|| {
                    if self.match_kind(&TokenKind::Comma) {
                        Some(self.previous_end(start))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| self.previous_end(start));
            members.push(TypeMember {
                name,
                ty,
                span: Span { start, end },
            });
        }

        self.expect(&TokenKind::RightBrace, "closing object type");
        members
    }

    fn binary_operator(&self) -> Option<(BinaryOperator, u8)> {
        let kind = self.peek_kind()?;
        match kind {
            TokenKind::PipePipe => Some((BinaryOperator::LogicalOr, 1)),
            TokenKind::QuestionQuestion => Some((BinaryOperator::NullishCoalesce, 1)),
            TokenKind::AndAnd => Some((BinaryOperator::LogicalAnd, 2)),
            TokenKind::EqualsEquals => Some((BinaryOperator::Equal, 3)),
            TokenKind::StrictEquals => Some((BinaryOperator::StrictEqual, 3)),
            TokenKind::BangEquals => Some((BinaryOperator::NotEqual, 3)),
            TokenKind::StrictBangEquals => Some((BinaryOperator::StrictNotEqual, 3)),
            TokenKind::Less => Some((BinaryOperator::Less, 4)),
            TokenKind::LessEquals => Some((BinaryOperator::LessEqual, 4)),
            TokenKind::Greater => Some((BinaryOperator::Greater, 4)),
            TokenKind::GreaterEquals => Some((BinaryOperator::GreaterEqual, 4)),
            TokenKind::Plus => Some((BinaryOperator::Add, 5)),
            TokenKind::Minus => Some((BinaryOperator::Subtract, 5)),
            TokenKind::Star => Some((BinaryOperator::Multiply, 6)),
            TokenKind::Slash => Some((BinaryOperator::Divide, 6)),
            TokenKind::Percent => Some((BinaryOperator::Remainder, 6)),
            _ => None,
        }
    }

    fn assignment_operator(&self) -> Option<AssignmentOperator> {
        match self.peek_kind()? {
            TokenKind::Equals => Some(AssignmentOperator::Assign),
            TokenKind::PlusEquals => Some(AssignmentOperator::AddAssign),
            TokenKind::MinusEquals => Some(AssignmentOperator::SubtractAssign),
            TokenKind::StarEquals => Some(AssignmentOperator::MultiplyAssign),
            TokenKind::SlashEquals => Some(AssignmentOperator::DivideAssign),
            TokenKind::PercentEquals => Some(AssignmentOperator::RemainderAssign),
            _ => None,
        }
    }

    fn consume_identifier(&mut self, expected: &str) -> Option<String> {
        match self.advance() {
            Some(Token {
                kind: TokenKind::Identifier(name),
                ..
            }) => Some(name),
            Some(token) => {
                self.error_at(token.span, &format!("expected {expected}"));
                None
            }
            None => {
                self.error_here(&format!("expected {expected}"));
                None
            }
        }
    }

    fn consume_property_name(&mut self, expected: &str) -> Option<String> {
        match self.advance() {
            Some(Token {
                kind: TokenKind::Identifier(name),
                ..
            }) => Some(name),
            Some(Token {
                kind: TokenKind::StringLiteral(name),
                ..
            }) => Some(name),
            Some(Token {
                kind: TokenKind::NumberLiteral(name),
                ..
            }) => Some(name),
            Some(token) => {
                self.error_at(token.span, &format!("expected {expected}"));
                None
            }
            None => {
                self.error_here(&format!("expected {expected}"));
                None
            }
        }
    }

    fn consume_optional_semicolon(&mut self) -> Option<Position> {
        if self.match_kind(&TokenKind::Semicolon) {
            Some(self.previous_end(self.fallback_position()))
        } else {
            None
        }
    }

    fn expect(&mut self, expected: &TokenKind, label: &str) -> Option<Token> {
        if self.check(expected) {
            self.advance()
        } else {
            self.error_here(&format!("expected {label}"));
            None
        }
    }

    fn expect_any(&mut self, label: &str) -> Token {
        self.advance().unwrap_or_else(|| {
            let position = self.fallback_position();
            let span = Span {
                start: position,
                end: position,
            };
            self.error_at(span, &format!("expected {label}"));
            Token {
                kind: TokenKind::Semicolon,
                span,
            }
        })
    }

    fn match_kind(&mut self, expected: &TokenKind) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, expected: &TokenKind) -> bool {
        self.peek_kind()
            .is_some_and(|kind| discriminant(kind) == discriminant(expected))
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.cursor).cloned()?;
        self.cursor += 1;
        Some(token)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.cursor)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|token| &token.kind)
    }

    fn is_at_end(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    fn current_start(&self) -> Position {
        self.peek()
            .map_or_else(|| self.fallback_position(), |token| token.span.start)
    }

    fn previous_end(&self, fallback: Position) -> Position {
        self.cursor
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map_or(fallback, |token| token.span.end)
    }

    fn fallback_position(&self) -> Position {
        if let Some(token) = self.tokens.last() {
            token.span.end
        } else {
            Position {
                byte: 0,
                line: 1,
                column: 1,
            }
        }
    }

    fn error_expression(&mut self) -> Expression {
        let position = self.fallback_position();
        let span = Span {
            start: position,
            end: position,
        };
        self.error_at(span, "expected expression");
        Expression {
            kind: ExpressionKind::Error,
            span,
        }
    }

    fn error_type(&self, start: Position) -> TypeNode {
        TypeNode {
            kind: TypeKind::Named("<error>".into()),
            span: Span { start, end: start },
        }
    }

    fn error_here(&mut self, message: &str) {
        let position = self.current_start();
        self.error_at(
            Span {
                start: position,
                end: position,
            },
            message,
        );
    }

    fn error_at(&mut self, span: Span, message: &str) {
        self.diagnostics.push(ParseDiagnostic {
            message: message.into(),
            span,
        });
    }

    fn synchronize_member(&mut self) {
        while !self.is_at_end() {
            if self.match_kind(&TokenKind::Semicolon)
                || self.match_kind(&TokenKind::Comma)
                || self.check(&TokenKind::RightBrace)
            {
                return;
            }
            self.advance();
        }
    }
}

fn merge_statement_spans(body: &[Statement]) -> Option<Span> {
    let first = body.first()?;
    let last = body.last()?;
    Some(Span {
        start: first.span.start,
        end: last.span.end,
    })
}
