use logos::Logos;
use crate::error::AsmError;

/// Token types produced by the lexer.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")]
pub enum Token {
    /// Register r0-r31
    #[regex(r"[rR]([0-9]|[12][0-9]|3[01])", |lex| {
        let s = lex.slice();
        s[1..].parse::<u8>().unwrap()
    })]
    Register(u8),

    /// Decimal or hex number
    #[regex(r"-?0x[0-9a-fA-F]+", |lex| {
        let s = lex.slice();
        i64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0)
    })]
    #[regex(r"-?[0-9]+", |lex| {
        let s = lex.slice();
        s.parse().unwrap_or(0)
    })]
    Number(i64),

    /// Comma
    #[token(",")]
    Comma,

    /// Colon
    #[token(":")]
    Colon,

    /// Left bracket
    #[token("[")]
    LBracket,

    /// Right bracket
    #[token("]")]
    RBracket,

    /// Plus
    #[token("+")]
    Plus,

    /// Minus
    #[token("-")]
    Minus,

    /// Star
    #[token("*")]
    Star,

    /// At sign (for labels)
    #[token("@")]
    At,

    /// Dot (for directives and local labels)
    #[token(".")]
    Dot,

    /// Newline
    #[token("\n")]
    Newline,

    /// String literal (for .ascii)
    #[regex(r#""[^"]*""#, |lex| {
        let s = lex.slice();
        s[1..s.len()-1].to_string()
    })]
    StringLiteral(String),

    /// Identifier (mnemonic, label reference, etc.)
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_.]*", |lex| lex.slice().to_string())]
    Ident(String),

    /// Comment - skip to end of line
    #[regex(r";[^\n]*", logos::skip)]
    #[regex(r"#[^\n]*", logos::skip)]
    Comment,
}

/// A parsed instruction from the token stream.
#[derive(Debug, Clone)]
pub struct ParsedInstruction {
    pub label: Option<String>,
    pub mnemonic: String,
    pub operands: Vec<String>,
    pub line: usize,
}

/// Tokenize the source text into a list of tokens per line.
/// Returns a list of (line_number, tokens) pairs.
pub fn tokenize(source: &str) -> Vec<(usize, Vec<Token>)> {
    let mut lines: Vec<(usize, Vec<Token>)> = Vec::new();
    let mut current_line: usize = 1;
    let mut current_tokens: Vec<Token> = Vec::new();

    let mut lex = Token::lexer(source);

    while let Some(token) = lex.next() {
        match token {
            Ok(Token::Newline) => {
                if !current_tokens.is_empty() {
                    lines.push((current_line, std::mem::take(&mut current_tokens)));
                }
                current_line += 1;
            }
            Ok(Token::Comment) => {
                // Comments are skipped by logos, but we account for the newline
                // Logos skips the comment text, but the semicolon is the Newline token
            }
            Ok(tok) => {
                current_tokens.push(tok);
            }
            Err(_) => {
                // Skip unrecognized tokens
            }
        }
    }

    // Don't forget the last line
    if !current_tokens.is_empty() {
        lines.push((current_line, current_tokens));
    }

    lines
}

/// Parse tokens into a list of instructions.
pub fn parse(tokens: &[(usize, Vec<Token>)]) -> Result<Vec<ParsedInstruction>, AsmError> {
    let mut instructions = Vec::new();
    let mut current_label: Option<String> = None;

    for (line_num, line_tokens) in tokens {
        if line_tokens.is_empty() {
            continue;
        }

        let mut idx: usize = 0;
        let tokens = line_tokens;

        // Check for label patterns:
        // 1. Ident Colon          → "label:"
        // 2. Dot Ident Colon      → ".label:"
        // 3. At Ident Colon       → "@label:"
        if idx + 1 < tokens.len() && matches!(&tokens[idx], Token::Ident(_))
            && matches!(&tokens[idx + 1], Token::Colon)
        {
            if let Token::Ident(label) = &tokens[idx] {
                current_label = Some(label.clone());
                idx += 2;
            }
        } else if idx + 2 < tokens.len() && matches!(&tokens[idx], Token::Dot)
            && matches!(&tokens[idx + 1], Token::Ident(_))
            && matches!(&tokens[idx + 2], Token::Colon)
        {
            if let Token::Ident(label) = &tokens[idx + 1] {
                current_label = Some(format!(".{}", label));
                idx += 3;
            }
        } else if idx + 2 < tokens.len() && matches!(&tokens[idx], Token::At)
            && matches!(&tokens[idx + 1], Token::Ident(_))
            && matches!(&tokens[idx + 2], Token::Colon)
        {
            if let Token::Ident(label) = &tokens[idx + 1] {
                current_label = Some(format!("@{}", label));
                idx += 3;
            }
        }

        // If we consumed the whole line (just a label), skip
        if idx >= tokens.len() {
            continue;
        }

        // The next token should be a mnemonic.
        // It can be an Ident, or Dot + Ident (directive)
        let mnemonic = match &tokens[idx] {
            Token::Ident(s) => {
                idx += 1;
                s.clone()
            }
            Token::Dot => {
                // Directive: .word, .byte, etc.
                if idx + 1 < tokens.len() {
                    if let Token::Ident(s) = &tokens[idx + 1] {
                        idx += 2;
                        format!(".{}", s)
                    } else {
                        return Err(AsmError::ParseError {
                            line: *line_num,
                            msg: format!("expected identifier after '.', got {:?}", tokens[idx + 1]),
                        });
                    }
                } else {
                    return Err(AsmError::ParseError {
                        line: *line_num,
                        msg: "unexpected end of line after '.'".to_string(),
                    });
                }
            }
            _ => {
                return Err(AsmError::ParseError {
                    line: *line_num,
                    msg: format!("expected mnemonic, got {:?}", tokens[idx]),
                });
            }
        };

        // Collect operands
        let operands = collect_operands(tokens, &mut idx, *line_num)?;

        instructions.push(ParsedInstruction {
            label: current_label,
            mnemonic,
            operands,
            line: *line_num,
        });
        current_label = None;
    }

    Ok(instructions)
}

/// Collect operands from the token stream starting at idx.
fn collect_operands(
    tokens: &[Token],
    idx: &mut usize,
    _line: usize,
) -> Result<Vec<String>, AsmError> {
    let mut operands = Vec::new();

    while *idx < tokens.len() {
        match &tokens[*idx] {
            Token::Comma => {
                *idx += 1;
                continue;
            }
            Token::LBracket => {
                // Collect memory operand: [reg + offset] or [reg + reg*scale + offset]
                let mut parts = Vec::new();
                *idx += 1;
                while *idx < tokens.len() {
                    if matches!(&tokens[*idx], Token::RBracket) {
                        *idx += 1;
                        break;
                    }
                    parts.push(token_to_string(&tokens[*idx]));
                    *idx += 1;
                }
                operands.push(format!("[{}]", parts.join(" ")));
            }
            Token::Minus => {
                // Standalone minus followed by a number = negative immediate
                if *idx + 1 < tokens.len() && matches!(&tokens[*idx + 1], Token::Number(_)) {
                    if let Token::Number(n) = &tokens[*idx + 1] {
                        operands.push(format!("-{}", n));
                        *idx += 2;
                        continue;
                    }
                }
                operands.push(token_to_string(&tokens[*idx]));
                *idx += 1;
            }
            Token::Dot => {
                // Dot followed by Ident = label reference (e.g., .L_test_alu)
                if *idx + 1 < tokens.len() && matches!(&tokens[*idx + 1], Token::Ident(_)) {
                    if let Token::Ident(s) = &tokens[*idx + 1] {
                        operands.push(format!(".{}", s));
                        *idx += 2;
                        continue;
                    }
                }
                operands.push(token_to_string(&tokens[*idx]));
                *idx += 1;
            }
            Token::At => {
                // At followed by Ident = label reference (e.g., @label)
                if *idx + 1 < tokens.len() && matches!(&tokens[*idx + 1], Token::Ident(_)) {
                    if let Token::Ident(s) = &tokens[*idx + 1] {
                        operands.push(format!("@{}", s));
                        *idx += 2;
                        continue;
                    }
                }
                operands.push(token_to_string(&tokens[*idx]));
                *idx += 1;
            }
            _ => {
                operands.push(token_to_string(&tokens[*idx]));
                *idx += 1;
            }
        }
    }

    Ok(operands)
}

fn token_to_string(tok: &Token) -> String {
    match tok {
        Token::Register(n) => format!("r{}", n),
        Token::Number(n) => format!("{}", n),
        Token::Ident(s) => s.clone(),
        Token::StringLiteral(s) => s.clone(),
        Token::Comma => ",".to_string(),
        Token::Colon => ":".to_string(),
        Token::LBracket => "[".to_string(),
        Token::RBracket => "]".to_string(),
        Token::Plus => "+".to_string(),
        Token::Minus => "-".to_string(),
        Token::Star => "*".to_string(),
        Token::At => "@".to_string(),
        Token::Dot => ".".to_string(),
        Token::Newline => "\n".to_string(),
        Token::Comment => unreachable!(),
    }
}