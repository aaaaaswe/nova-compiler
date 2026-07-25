use logos::Logos;

/// All tokens in the Nova language.
#[derive(Logos, Debug, PartialEq, Clone)]
pub enum Token {
    // ── Keywords ──
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("fn")]
    Fn,
    #[token("struct")]
    Struct,
    #[token("enum")]
    Enum,
    #[token("union")]
    Union,
    #[token("impl")]
    Impl,
    #[token("mod")]
    Mod,
    #[token("use")]
    Use,
    #[token("pub")]
    Pub,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("loop")]
    Loop,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("return")]
    Return,
    #[token("unsafe")]
    Unsafe,
    #[token("asm")]
    Asm,
    #[token("extern")]
    Extern,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("as")]
    As,
    #[token("self")]
    Self_,
    #[token("sizeof")]
    Sizeof,
    #[token("alignof")]
    Alignof,
    #[token("defer")]
    Defer,
    #[token("const")]
    Const,

    // ── Literals ──
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    IntLiteral(i64),
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().ok())]
    FloatLiteral(f64),
    #[regex(r#""[^"]*""#, |lex| {
        let s = lex.slice();
        s[1..s.len() - 1].to_string()
    })]
    StringLiteral(String),

    // ── Identifiers (must come after keywords) ──
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    // ── Symbols ──
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(";")]
    Semicolon,
    #[token(":")]
    Colon,
    #[token("::")]
    PathSep,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("...")]
    Ellipsis,
    #[token("..=")]
    DotDotEq,
    #[token("..")]
    DotDot,
    #[token("->")]
    Arrow,
    #[token("=")]
    Eq,
    #[token("==")]
    EqEq,
    #[token("!=")]
    Ne,
    #[token("<")]
    Lt,
    #[token("<=")]
    Le,
    #[token(">")]
    Gt,
    #[token(">=")]
    Ge,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("&")]
    Amp,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token("!")]
    Bang,
    #[token("<<")]
    Shl,
    #[token(">>")]
    Shr,
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,
    #[token("&=")]
    AmpEq,
    #[token("|=")]
    PipeEq,
    #[token("@")]
    At,
    #[token("#")]
    Hash,
    #[token("&mut")]
    AmpMut,
    #[token("_", priority = 3)]
    Underscore,

    // ── Skipped tokens ──
    #[regex(r"//[^\n]*", logos::skip)]
    #[regex(r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/", logos::skip)]
    #[regex(r"[ \t\n\r\f]+", logos::skip)]
    Whitespace,
}

impl Token {
    /// Return a human-readable name for the token (for error messages).
    pub fn name(&self) -> String {
        match self {
            Token::Let => "let".into(),
            Token::Mut => "mut".into(),
            Token::Fn => "fn".into(),
            Token::Struct => "struct".into(),
            Token::Enum => "enum".into(),
            Token::Union => "union".into(),
            Token::Impl => "impl".into(),
            Token::Mod => "mod".into(),
            Token::Use => "use".into(),
            Token::Pub => "pub".into(),
            Token::If => "if".into(),
            Token::Else => "else".into(),
            Token::While => "while".into(),
            Token::For => "for".into(),
            Token::In => "in".into(),
            Token::Loop => "loop".into(),
            Token::Break => "break".into(),
            Token::Continue => "continue".into(),
            Token::Return => "return".into(),
            Token::Unsafe => "unsafe".into(),
            Token::Asm => "asm".into(),
            Token::Extern => "extern".into(),
            Token::True => "true".into(),
            Token::False => "false".into(),
            Token::As => "as".into(),
            Token::Self_ => "self".into(),
            Token::Sizeof => "sizeof".into(),
            Token::Alignof => "alignof".into(),
            Token::Defer => "defer".into(),
            Token::Const => "const".into(),
            Token::IntLiteral(n) => format!("{}", n),
            Token::FloatLiteral(f) => format!("{}", f),
            Token::StringLiteral(s) => format!("\"{}\"", s),
            Token::Ident(s) => s.clone(),
            Token::LParen => "(".into(),
            Token::RParen => ")".into(),
            Token::LBrace => "{".into(),
            Token::RBrace => "}".into(),
            Token::LBracket => "[".into(),
            Token::RBracket => "]".into(),
            Token::Semicolon => ";".into(),
            Token::Colon => ":".into(),
            Token::PathSep => "::".into(),
            Token::Comma => ",".into(),
            Token::Dot => ".".into(),
            Token::Ellipsis => "...".into(),
            Token::DotDotEq => "..=".into(),
            Token::DotDot => "..".into(),
            Token::Arrow => "->".into(),
            Token::Eq => "=".into(),
            Token::EqEq => "==".into(),
            Token::Ne => "!=".into(),
            Token::Lt => "<".into(),
            Token::Le => "<=".into(),
            Token::Gt => ">".into(),
            Token::Ge => ">=".into(),
            Token::Plus => "+".into(),
            Token::Minus => "-".into(),
            Token::Star => "*".into(),
            Token::Slash => "/".into(),
            Token::Percent => "%".into(),
            Token::Amp => "&".into(),
            Token::Pipe => "|".into(),
            Token::Caret => "^".into(),
            Token::Bang => "!".into(),
            Token::Shl => "<<".into(),
            Token::Shr => ">>".into(),
            Token::PlusEq => "+=".into(),
            Token::MinusEq => "-=".into(),
            Token::StarEq => "*=".into(),
            Token::SlashEq => "/=".into(),
            Token::AmpEq => "&=".into(),
            Token::PipeEq => "|=".into(),
            Token::At => "@".into(),
            Token::Hash => "#".into(),
            Token::AmpMut => "&mut".into(),
            Token::Underscore => "_".into(),
            Token::Whitespace => "<whitespace>".into(),
        }
    }
}

/// Tokenize source code into a vector of (Token, Span) pairs.
pub fn tokenize(source: &str) -> Vec<(Token, std::ops::Range<usize>)> {
    let lexer = Token::lexer(source);
    lexer
        .spanned()
        .map(|(token, span)| (token.unwrap_or(Token::Whitespace), span))
        .filter(|(t, _)| !matches!(t, Token::Whitespace))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keywords() {
        let tokens = tokenize("let mut fn struct enum union impl mod use pub");
        let expected: Vec<Token> = vec![
            Token::Let,
            Token::Mut,
            Token::Fn,
            Token::Struct,
            Token::Enum,
            Token::Union,
            Token::Impl,
            Token::Mod,
            Token::Use,
            Token::Pub,
        ];
        let result: Vec<Token> = tokens.into_iter().map(|(t, _)| t).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_literals() {
        let tokens = tokenize(r#"42 3.14 "hello" true false"#);
        let expected: Vec<Token> = vec![
            Token::IntLiteral(42),
            Token::FloatLiteral(3.14),
            Token::StringLiteral("hello".into()),
            Token::True,
            Token::False,
        ];
        let result: Vec<Token> = tokens.into_iter().map(|(t, _)| t).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_symbols() {
        let tokens = tokenize("( ) { } [ ] ; : :: , . .. ..= -> = == != < <= > >= + - * / % & | ^ ! << >>");
        let result: Vec<Token> = tokens.into_iter().map(|(t, _)| t).collect();
        let expected: Vec<Token> = vec![
            Token::LParen,
            Token::RParen,
            Token::LBrace,
            Token::RBrace,
            Token::LBracket,
            Token::RBracket,
            Token::Semicolon,
            Token::Colon,
            Token::PathSep,
            Token::Comma,
            Token::Dot,
            Token::DotDot,
            Token::DotDotEq,
            Token::Arrow,
            Token::Eq,
            Token::EqEq,
            Token::Ne,
            Token::Lt,
            Token::Le,
            Token::Gt,
            Token::Ge,
            Token::Plus,
            Token::Minus,
            Token::Star,
            Token::Slash,
            Token::Percent,
            Token::Amp,
            Token::Pipe,
            Token::Caret,
            Token::Bang,
            Token::Shl,
            Token::Shr,
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_comments() {
        let tokens = tokenize("// comment\nlet /* block */ x");
        let result: Vec<Token> = tokens.into_iter().map(|(t, _)| t).collect();
        assert_eq!(result, vec![Token::Let, Token::Ident("x".into())]);
    }

    #[test]
    fn test_complex() {
        let source = "fn add(a: i64, b: i64) -> i64 { return a + b; }";
        let tokens = tokenize(source);
        let result: Vec<Token> = tokens.into_iter().map(|(t, _)| t).collect();
        let expected: Vec<Token> = vec![
            Token::Fn,
            Token::Ident("add".into()),
            Token::LParen,
            Token::Ident("a".into()),
            Token::Colon,
            Token::Ident("i64".into()),
            Token::Comma,
            Token::Ident("b".into()),
            Token::Colon,
            Token::Ident("i64".into()),
            Token::RParen,
            Token::Arrow,
            Token::Ident("i64".into()),
            Token::LBrace,
            Token::Return,
            Token::Ident("a".into()),
            Token::Plus,
            Token::Ident("b".into()),
            Token::Semicolon,
            Token::RBrace,
        ];
        assert_eq!(result, expected);
    }
}