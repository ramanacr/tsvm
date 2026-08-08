use tsvm_lexer::{lex, LexErrorKind, TokenKind};

fn kinds(source: &str) -> Vec<TokenKind> {
    lex(source)
        .expect("source should lex")
        .into_iter()
        .map(|token| token.kind)
        .collect()
}

#[test]
fn tokenizes_initial_demo_program_subset() {
    let source = r#"
interface Account {
  id: number;
  balance: number;
}

function credit(account: Account, amount: number): number {
  account.balance += amount;
  return account.balance;
}

const account: Account = {
  id: 1,
  balance: 100
};

console.log(credit(account, 50));
"#;

    assert_eq!(
        kinds(source),
        vec![
            TokenKind::Interface,
            TokenKind::Identifier("Account".into()),
            TokenKind::LeftBrace,
            TokenKind::Identifier("id".into()),
            TokenKind::Colon,
            TokenKind::NumberType,
            TokenKind::Semicolon,
            TokenKind::Identifier("balance".into()),
            TokenKind::Colon,
            TokenKind::NumberType,
            TokenKind::Semicolon,
            TokenKind::RightBrace,
            TokenKind::Function,
            TokenKind::Identifier("credit".into()),
            TokenKind::LeftParen,
            TokenKind::Identifier("account".into()),
            TokenKind::Colon,
            TokenKind::Identifier("Account".into()),
            TokenKind::Comma,
            TokenKind::Identifier("amount".into()),
            TokenKind::Colon,
            TokenKind::NumberType,
            TokenKind::RightParen,
            TokenKind::Colon,
            TokenKind::NumberType,
            TokenKind::LeftBrace,
            TokenKind::Identifier("account".into()),
            TokenKind::Dot,
            TokenKind::Identifier("balance".into()),
            TokenKind::PlusEquals,
            TokenKind::Identifier("amount".into()),
            TokenKind::Semicolon,
            TokenKind::Return,
            TokenKind::Identifier("account".into()),
            TokenKind::Dot,
            TokenKind::Identifier("balance".into()),
            TokenKind::Semicolon,
            TokenKind::RightBrace,
            TokenKind::Const,
            TokenKind::Identifier("account".into()),
            TokenKind::Colon,
            TokenKind::Identifier("Account".into()),
            TokenKind::Equals,
            TokenKind::LeftBrace,
            TokenKind::Identifier("id".into()),
            TokenKind::Colon,
            TokenKind::NumberLiteral("1".into()),
            TokenKind::Comma,
            TokenKind::Identifier("balance".into()),
            TokenKind::Colon,
            TokenKind::NumberLiteral("100".into()),
            TokenKind::RightBrace,
            TokenKind::Semicolon,
            TokenKind::Identifier("console".into()),
            TokenKind::Dot,
            TokenKind::Identifier("log".into()),
            TokenKind::LeftParen,
            TokenKind::Identifier("credit".into()),
            TokenKind::LeftParen,
            TokenKind::Identifier("account".into()),
            TokenKind::Comma,
            TokenKind::NumberLiteral("50".into()),
            TokenKind::RightParen,
            TokenKind::RightParen,
            TokenKind::Semicolon,
        ]
    );
}

#[test]
fn reports_byte_offsets_and_line_columns() {
    let tokens = lex("let\nanswer: number = 42;").expect("source should lex");

    assert_eq!(tokens[1].kind, TokenKind::Identifier("answer".into()));
    assert_eq!(tokens[1].span.start.byte, 4);
    assert_eq!(tokens[1].span.start.line, 2);
    assert_eq!(tokens[1].span.start.column, 1);
    assert_eq!(tokens[1].span.end.byte, 10);
    assert_eq!(tokens[1].span.end.line, 2);
    assert_eq!(tokens[1].span.end.column, 7);
}

#[test]
fn skips_comments_and_whitespace() {
    assert_eq!(
        kinds("/* header */\nlet x = 1; // trailing\nconst y = x + 2;"),
        vec![
            TokenKind::Let,
            TokenKind::Identifier("x".into()),
            TokenKind::Equals,
            TokenKind::NumberLiteral("1".into()),
            TokenKind::Semicolon,
            TokenKind::Const,
            TokenKind::Identifier("y".into()),
            TokenKind::Equals,
            TokenKind::Identifier("x".into()),
            TokenKind::Plus,
            TokenKind::NumberLiteral("2".into()),
            TokenKind::Semicolon,
        ]
    );
}

#[test]
fn tokenizes_v0_control_flow_operators_and_literals() {
    assert_eq!(
        kinds(r#"if (ok && value !== null) { throw "bad"; } else { value ?? undefined; }"#),
        vec![
            TokenKind::If,
            TokenKind::LeftParen,
            TokenKind::Identifier("ok".into()),
            TokenKind::AndAnd,
            TokenKind::Identifier("value".into()),
            TokenKind::StrictBangEquals,
            TokenKind::Null,
            TokenKind::RightParen,
            TokenKind::LeftBrace,
            TokenKind::Throw,
            TokenKind::StringLiteral("bad".into()),
            TokenKind::Semicolon,
            TokenKind::RightBrace,
            TokenKind::Else,
            TokenKind::LeftBrace,
            TokenKind::Identifier("value".into()),
            TokenKind::QuestionQuestion,
            TokenKind::Undefined,
            TokenKind::Semicolon,
            TokenKind::RightBrace,
        ]
    );
}

#[test]
fn rejects_unterminated_string() {
    let err = lex(r#"const s = "open"#).expect_err("unterminated string should fail");

    assert_eq!(err.kind, LexErrorKind::UnterminatedString);
    assert_eq!(err.span.start.line, 1);
    assert_eq!(err.span.start.column, 11);
}

#[test]
fn rejects_unterminated_block_comment() {
    let err = lex("/* open").expect_err("unterminated block comment should fail");

    assert_eq!(err.kind, LexErrorKind::UnterminatedBlockComment);
    assert_eq!(err.span.start.byte, 0);
}
