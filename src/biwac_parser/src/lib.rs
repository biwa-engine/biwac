pub mod macros;
pub mod symbols;
pub mod types;

#[cfg(test)]
mod tests;

use biwac_ast::{Ident, QualifiedId};
use biwac_base::Span;
use biwac_lexer::{TkKind, Token};

pub(crate) use symbols::statements::ExprOrStmt;

#[derive(Debug, Clone)]
pub enum ParseError {
    InvalidToken(Vec<TkKind>, Token), // expected TokenKind, ... or TokenKind, but found Token in Token.range
    InvalidEOF(Vec<TkKind>),          // expected TokenKind, ... or TokenKind, but found EOF
    StructMemberConflict(String, String, Box<Token>), // struct id, member id
}

#[derive(Debug, Clone)]
pub struct Parser {
    tokens: Vec<Token>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens }
    }
}

#[derive(Debug)]
pub(crate) struct TokenStream<'t> {
    tokens: std::iter::Peekable<std::slice::Iter<'t, Token>>,
}

impl<'t> TokenStream<'t> {
    pub(crate) fn new(tokens: std::iter::Peekable<std::slice::Iter<'t, Token>>) -> Self {
        Self { tokens }
    }

    pub(crate) fn next(&mut self) -> Option<&Token> {
        self.tokens.next()
    }

    pub(crate) fn peek(&mut self) -> Option<&&'t Token> {
        self.tokens.peek()
    }

    pub(crate) fn consume_next_if_match(&mut self, kinds: Vec<TkKind>) -> Option<&Token> {
        let t: &Token = self.peek()?;

        for kind in &kinds {
            if kind == &t.kind {
                self.next();
                return Some(t);
            }
        }

        None
    }

    pub(crate) fn must_consume_next(&mut self, kinds: Vec<TkKind>) -> Result<&Token, ParseError> {
        let t: &Token = self.next().ok_or(ParseError::InvalidEOF(kinds.clone()))?;

        for kind in &kinds {
            if kind == &t.kind {
                return Ok(t);
            }
        }

        Err(ParseError::InvalidToken(kinds, t.clone()))
    }

    pub(crate) fn must_consume_semicolon(&mut self) -> Result<Token, ParseError> {
        let t = self
            .next()
            .ok_or(ParseError::InvalidEOF(vec![TkKind::SemiColon]))?
            .clone();

        if t.kind == TkKind::SemiColon {
            Ok(t)
        } else {
            Err(ParseError::InvalidToken(vec![TkKind::SemiColon], t.clone()))
        }
    }

    pub(crate) fn opt_consume_semicolon(&mut self) -> Option<Token> {
        if let Some(t) = self.peek().cloned()
            && t.kind == TkKind::SemiColon
        {
            self.next();
            Some(t.to_owned())
        } else {
            None
        }
    }

    pub(crate) fn consume_identifier(&mut self) -> Result<Ident, ParseError> {
        let t = self
            .next()
            .ok_or(ParseError::InvalidEOF(vec![TkKind::Ident]))?;

        if let TkKind::Ident = &t.kind {
            Ok(Ident {
                id: t.unwrap_string_value(),
                span: t.span.clone(),
            })
        } else {
            Err(ParseError::InvalidToken(vec![TkKind::Ident], t.clone()))
        }
    }

    pub(crate) fn consume_qualified_identifier(&mut self) -> Result<QualifiedId, ParseError> {
        let mut ids = vec![];
        let (is_from_root, begin, mut end) = if let Some(t) = self.peek().cloned()
            && matches!(t.kind, TkKind::Package)
        {
            self.next();
            self.must_consume_next(vec![TkKind::DoubleColon])?;

            ids.push(self.consume_identifier()?.id);

            (true, t.span.clone(), t.span.clone())
        } else {
            let ident = self.consume_identifier()?;
            ids.push(ident.id);

            (false, ident.span.clone(), ident.span)
        };

        loop {
            if let Some(t) = self.peek() {
                if let TkKind::DoubleColon = t.kind {
                    self.next();
                    let ident = self.consume_identifier()?;
                    ids.push(ident.id);
                    end = ident.span;
                } else {
                    return Ok(QualifiedId {
                        is_from_root,
                        quals: ids[..ids.len() - 1].to_vec(),
                        id: ids.last().expect("no identifier parsed").clone(),
                        span: Span::merge(&begin, &end),
                    });
                }
            } else {
                return Ok(QualifiedId {
                    is_from_root,
                    quals: ids[..ids.len() - 1].to_vec(),
                    id: ids.last().expect("no identifier parsed").clone(),
                    span: Span::merge(&begin, &end),
                });
            }
        }
    }
}

//
// impl ParseError {
//     pub fn panic_with_error_message(&self, src: &str) -> ! {
//         eprint!("\x1b[1;38;2;255;20;0merror\x1b[m: ");
//         match self {
//             Self::InvalidToken(expecteds, found) => {
//                 eprintln!(
//                     "\x1b[1mexpected {}, but found {} in the {}th character\x1b[m",
//                     string_of(expecteds),
//                     found.kind,
//                     found.range.begin
//                 );
//                 eprintln!(
//                     "... {}\x1b[4m{}\x1b[m{} ...",
//                     &src[0.max(found.range.begin - 10)..found.range.begin],
//                     &src[found.range.begin..found.range.end],
//                     &src[found.range.end..src.len().min(found.range.end + 10)],
//                 );
//             }
//             Self::InvalidEOF(expecteds) => {
//                 eprintln!(
//                     "\x1b[1mexpected {}, but found EOF\x1b[m",
//                     string_of(expecteds),
//                 );
//                 eprintln!(
//                     "... {}\x1b[4m{}\x1b[m",
//                     &src[0.max(src.len() - 20)..0.max(src.len() - 5)],
//                     &src[0.max(src.len() - 5)..],
//                 );
//             }
//             Self::StructMemberConflict(structid, memberid, range) => {
//                 eprintln!("\x1b[1mmember `{memberid}` in struct `{structid}` is conflicted\x1b[m",);
//                 eprintln!(
//                     "... {}\x1b[4m{}\x1b[m{} ...",
//                     &src[0.max(range.begin - 10)..range.begin],
//                     &src[range.begin..range.end],
//                     &src[range.end..src.len().min(range.end + 10)],
//                 );
//             }
//         }
//
//         panic!("");
//     }
// }
//
// impl Display for TokenKind {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             Self::Identifier(s) => {
//                 if s.is_empty() {
//                     write!(f, "identifier")
//                 } else {
//                     write!(f, "identifier `{s}`")
//                 }
//             }
//             Self::IntLiteral(i) => write!(f, "literal `{i}`"),
//             Self::CharLiteral(c) => write!(f, "literal `'{c}'`"),
//             Self::StringLiteral(s) => write!(f, "literal `\"{s}\"`"),
//             _ => write!(f, "`{}`", self.pattern()),
//         }
//     }
// }
//
// fn string_of(kinds: &[TokenKind]) -> String {
//     if kinds.is_empty() {
//         "".to_string()
//     } else if kinds.len() == 1 {
//         kinds.first().unwrap().to_string()
//     } else if kinds.len() == 2 {
//         format!("{} or {}", kinds.first().unwrap(), kinds.last().unwrap())
//     } else {
//         format!(
//             "{}{} or {}",
//             &kinds[..kinds.len() - 2]
//                 .iter()
//                 .map(|kind| format!("{kind}, "))
//                 .collect::<String>(),
//             &kinds[kinds.len() - 2..].first().unwrap(),
//             &kinds[kinds.len() - 2..].last().unwrap()
//         )
//     }
// }
