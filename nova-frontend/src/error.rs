use thiserror::Error;

/// Source location span (byte offsets into source text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Parse error with source location information.
#[derive(Error, Debug, Clone)]
pub enum ParseError {
    #[error("unexpected token '{token}' at line {line}, column {column}")]
    UnexpectedToken {
        token: String,
        line: usize,
        column: usize,
    },

    #[error("expected {expected} at line {line}, column {column}, found '{found}'")]
    ExpectedToken {
        expected: String,
        found: String,
        line: usize,
        column: usize,
    },

    #[error("unexpected end of input")]
    UnexpectedEof,

    #[error("{message} at line {line}, column {column}")]
    Generic {
        message: String,
        line: usize,
        column: usize,
    },
}

impl ParseError {
    /// Create a new unexpected token error from a span and source text.
    pub fn unexpected_token(token: &str, span: Span, source: &str) -> Self {
        let (line, column) = span_to_line_col(span.start, source);
        ParseError::UnexpectedToken {
            token: token.to_string(),
            line,
            column,
        }
    }

    /// Create a new expected token error.
    pub fn expected_token(
        expected: &str,
        found: &str,
        span: Span,
        source: &str,
    ) -> Self {
        let (line, column) = span_to_line_col(span.start, source);
        ParseError::ExpectedToken {
            expected: expected.to_string(),
            found: found.to_string(),
            line,
            column,
        }
    }

    /// Create a new generic error with location.
    pub fn generic(message: &str, span: Span, source: &str) -> Self {
        let (line, column) = span_to_line_col(span.start, source);
        ParseError::Generic {
            message: message.to_string(),
            line,
            column,
        }
    }
}

/// Convert a byte offset to (line, column) using the source text.
/// Lines are 1-indexed, columns are 1-indexed.
pub fn span_to_line_col(offset: usize, source: &str) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;

    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
}

/// Precompute the start byte offset of each line in the source.
pub fn compute_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(i + 1);
        }
    }
    starts
}