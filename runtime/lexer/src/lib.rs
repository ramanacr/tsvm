#![forbid(unsafe_code)]

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier(String),
    NumberLiteral(String),
    StringLiteral(String),
    BooleanLiteral(bool),
    Let,
    Const,
    Var,
    Function,
    Interface,
    Type,
    Class,
    Import,
    Export,
    From,
    If,
    Else,
    Switch,
    Case,
    Default,
    For,
    While,
    Do,
    Try,
    Catch,
    Finally,
    Throw,
    Return,
    New,
    As,
    Extends,
    Implements,
    NumberType,
    StringType,
    BooleanType,
    Null,
    Undefined,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Equals,
    PlusEquals,
    MinusEquals,
    StarEquals,
    SlashEquals,
    PercentEquals,
    EqualsEquals,
    StrictEquals,
    BangEquals,
    StrictBangEquals,
    Bang,
    Less,
    LessEquals,
    Greater,
    GreaterEquals,
    AndAnd,
    PipePipe,
    QuestionQuestion,
    Arrow,
    Colon,
    Semicolon,
    Comma,
    Dot,
    Question,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Position {
    pub byte: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexErrorKind {
    UnexpectedCharacter(char),
    UnterminatedString,
    UnterminatedBlockComment,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            LexErrorKind::UnexpectedCharacter(ch) => write!(
                f,
                "unexpected character `{ch}` at {}:{}",
                self.span.start.line, self.span.start.column
            ),
            LexErrorKind::UnterminatedString => write!(
                f,
                "unterminated string at {}:{}",
                self.span.start.line, self.span.start.column
            ),
            LexErrorKind::UnterminatedBlockComment => write!(
                f,
                "unterminated block comment at {}:{}",
                self.span.start.line, self.span.start.column
            ),
        }
    }
}

impl std::error::Error for LexError {}

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(source).lex_all()
}

struct Lexer<'source> {
    source: &'source str,
    cursor: usize,
    line: usize,
    column: usize,
}

impl<'source> Lexer<'source> {
    fn new(source: &'source str) -> Self {
        Self {
            source,
            cursor: 0,
            line: 1,
            column: 1,
        }
    }

    fn lex_all(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        while !self.is_at_end() {
            self.skip_trivia()?;

            if self.is_at_end() {
                break;
            }

            tokens.push(self.next_token()?);
        }

        Ok(tokens)
    }

    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(ch) if ch.is_whitespace() => {
                    self.advance();
                }
                Some('/') if self.peek_next() == Some('/') => {
                    self.advance();
                    self.advance();
                    while self.peek().is_some_and(|ch| ch != '\n') {
                        self.advance();
                    }
                }
                Some('/') if self.peek_next() == Some('*') => {
                    self.skip_block_comment()?;
                }
                _ => return Ok(()),
            }
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), LexError> {
        let start = self.position();
        self.advance();
        self.advance();

        while let Some(ch) = self.peek() {
            if ch == '*' && self.peek_next() == Some('/') {
                self.advance();
                self.advance();
                return Ok(());
            }

            self.advance();
        }

        Err(LexError {
            kind: LexErrorKind::UnterminatedBlockComment,
            span: Span {
                start,
                end: self.position(),
            },
        })
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        let start = self.position();
        let ch = self.advance().expect("next_token called at EOF");
        let kind = match ch {
            'a'..='z' | 'A'..='Z' | '_' | '$' => self.identifier_or_keyword(start),
            '0'..='9' => self.number_literal(start),
            '"' | '\'' => self.string_literal(start, ch)?,
            '+' => self.match_or(TokenKind::PlusEquals, TokenKind::Plus, '='),
            '-' => self.match_or(TokenKind::MinusEquals, TokenKind::Minus, '='),
            '*' => self.match_or(TokenKind::StarEquals, TokenKind::Star, '='),
            '/' => self.match_or(TokenKind::SlashEquals, TokenKind::Slash, '='),
            '%' => self.match_or(TokenKind::PercentEquals, TokenKind::Percent, '='),
            '=' if self.match_char('>') => TokenKind::Arrow,
            '=' if self.match_char('=') => {
                if self.match_char('=') {
                    TokenKind::StrictEquals
                } else {
                    TokenKind::EqualsEquals
                }
            }
            '=' => TokenKind::Equals,
            '!' if self.match_char('=') => {
                if self.match_char('=') {
                    TokenKind::StrictBangEquals
                } else {
                    TokenKind::BangEquals
                }
            }
            '!' => TokenKind::Bang,
            '<' => self.match_or(TokenKind::LessEquals, TokenKind::Less, '='),
            '>' => self.match_or(TokenKind::GreaterEquals, TokenKind::Greater, '='),
            '&' if self.match_char('&') => TokenKind::AndAnd,
            '|' if self.match_char('|') => TokenKind::PipePipe,
            '?' if self.match_char('?') => TokenKind::QuestionQuestion,
            '?' => TokenKind::Question,
            ':' => TokenKind::Colon,
            ';' => TokenKind::Semicolon,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            '(' => TokenKind::LeftParen,
            ')' => TokenKind::RightParen,
            '{' => TokenKind::LeftBrace,
            '}' => TokenKind::RightBrace,
            '[' => TokenKind::LeftBracket,
            ']' => TokenKind::RightBracket,
            unexpected => {
                return Err(LexError {
                    kind: LexErrorKind::UnexpectedCharacter(unexpected),
                    span: Span {
                        start,
                        end: self.position(),
                    },
                })
            }
        };

        Ok(Token {
            kind,
            span: Span {
                start,
                end: self.position(),
            },
        })
    }

    fn identifier_or_keyword(&mut self, start: Position) -> TokenKind {
        self.consume_while(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$');
        let text = &self.source[start.byte..self.cursor];

        match text {
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "var" => TokenKind::Var,
            "function" => TokenKind::Function,
            "interface" => TokenKind::Interface,
            "type" => TokenKind::Type,
            "class" => TokenKind::Class,
            "import" => TokenKind::Import,
            "export" => TokenKind::Export,
            "from" => TokenKind::From,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "switch" => TokenKind::Switch,
            "case" => TokenKind::Case,
            "default" => TokenKind::Default,
            "for" => TokenKind::For,
            "while" => TokenKind::While,
            "do" => TokenKind::Do,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "finally" => TokenKind::Finally,
            "throw" => TokenKind::Throw,
            "return" => TokenKind::Return,
            "new" => TokenKind::New,
            "as" => TokenKind::As,
            "extends" => TokenKind::Extends,
            "implements" => TokenKind::Implements,
            "number" => TokenKind::NumberType,
            "string" => TokenKind::StringType,
            "boolean" => TokenKind::BooleanType,
            "true" => TokenKind::BooleanLiteral(true),
            "false" => TokenKind::BooleanLiteral(false),
            "null" => TokenKind::Null,
            "undefined" => TokenKind::Undefined,
            _ => TokenKind::Identifier(text.into()),
        }
    }

    fn number_literal(&mut self, start: Position) -> TokenKind {
        self.consume_while(|ch| ch.is_ascii_digit());

        if self.peek() == Some('.') && self.peek_next().is_some_and(|ch| ch.is_ascii_digit()) {
            self.advance();
            self.consume_while(|ch| ch.is_ascii_digit());
        }

        if self.peek().is_some_and(|ch| ch == 'e' || ch == 'E') {
            let exponent_start = self.cursor;
            let exponent_line = self.line;
            let exponent_column = self.column;
            self.advance();
            if self.peek().is_some_and(|ch| ch == '+' || ch == '-') {
                self.advance();
            }

            if self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.consume_while(|ch| ch.is_ascii_digit());
            } else {
                self.cursor = exponent_start;
                self.line = exponent_line;
                self.column = exponent_column;
            }
        }

        TokenKind::NumberLiteral(self.source[start.byte..self.cursor].into())
    }

    fn string_literal(&mut self, start: Position, quote: char) -> Result<TokenKind, LexError> {
        let mut value = String::new();

        while let Some(ch) = self.peek() {
            if ch == quote {
                self.advance();
                return Ok(TokenKind::StringLiteral(value));
            }

            if ch == '\n' || ch == '\r' {
                return Err(LexError {
                    kind: LexErrorKind::UnterminatedString,
                    span: Span {
                        start,
                        end: self.position(),
                    },
                });
            }

            if ch == '\\' {
                self.advance();
                let Some(escaped) = self.advance() else {
                    break;
                };
                value.push(match escaped {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '"' => '"',
                    '\'' => '\'',
                    '\\' => '\\',
                    other => other,
                });
                continue;
            }

            value.push(ch);
            self.advance();
        }

        Err(LexError {
            kind: LexErrorKind::UnterminatedString,
            span: Span {
                start,
                end: self.position(),
            },
        })
    }

    fn match_or(&mut self, matched: TokenKind, unmatched: TokenKind, expected: char) -> TokenKind {
        if self.match_char(expected) {
            matched
        } else {
            unmatched
        }
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        while let Some(ch) = self.peek() {
            if !predicate(ch) {
                break;
            }

            self.advance();
        }
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.cursor += ch.len_utf8();

        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        Some(ch)
    }

    fn peek(&self) -> Option<char> {
        self.source[self.cursor..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let mut chars = self.source[self.cursor..].chars();
        chars.next()?;
        chars.next()
    }

    fn is_at_end(&self) -> bool {
        self.cursor >= self.source.len()
    }

    fn position(&self) -> Position {
        Position {
            byte: self.cursor,
            line: self.line,
            column: self.column,
        }
    }
}
