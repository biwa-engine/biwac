mod error;
pub mod macros;
pub mod symbols;
pub mod types;

#[cfg(test)]
mod tests;

use biwac_ast::{Ident, QualifiedId};
use biwac_base::{ModPath, Span};
use biwac_lexer::{TkKind, TkKindName, Token};

pub(crate) use symbols::statements::ExprOrStmt;

pub use error::ParseError;

#[derive(Debug, Clone)]
pub struct Parser<'src> {
    modpath: ModPath,
    tokens: Vec<Token<'src>>,
}

impl<'src> Parser<'src> {
    pub fn new(modpath: ModPath, tokens: Vec<Token<'src>>) -> Self {
        Self { modpath, tokens }
    }
}

#[derive(Debug)]
pub(crate) struct TokenStream<'t, 'src> {
    tokens: std::iter::Peekable<std::slice::Iter<'t, Token<'src>>>,
}

impl<'t, 'src> TokenStream<'t, 'src> {
    pub(crate) fn new(tokens: std::iter::Peekable<std::slice::Iter<'t, Token<'src>>>) -> Self {
        Self { tokens }
    }

    pub(crate) fn next(&mut self) -> Option<&Token<'src>> {
        self.tokens.next()
    }

    pub(crate) fn peek(&mut self) -> Option<&&'t Token<'src>> {
        self.tokens.peek()
    }

    pub(crate) fn consume_next_if_match(&mut self, kinds: Vec<TkKindName>) -> Option<&Token<'src>> {
        let t: &Token = self.peek()?;

        for kind in &kinds {
            if kind == &t.kind.as_name() {
                self.next();
                return Some(t);
            }
        }

        None
    }

    pub(crate) fn must_consume_next(
        &mut self,
        expecteds: Vec<TkKindName>,
    ) -> Result<&Token<'src>, ParseError<'src>> {
        let t: &Token<'src> = self.next().ok_or(ParseError::InvalidEOF {
            expecteds: expecteds.clone(),
        })?;

        for kind in &expecteds {
            if *kind == t.kind.as_name() {
                return Ok(t);
            }
        }

        Err(ParseError::InvalidToken {
            expecteds,
            found: t.clone(),
        })
    }

    pub(crate) fn must_consume_semicolon(&mut self) -> Result<Token<'src>, ParseError<'src>> {
        let t = self
            .next()
            .ok_or(ParseError::InvalidEOF {
                expecteds: vec![TkKindName::MarkSemiColon],
            })?
            .clone();

        if t.kind == TkKind::MarkSemiColon {
            Ok(t)
        } else {
            Err(ParseError::InvalidToken {
                expecteds: vec![TkKindName::MarkSemiColon],
                found: t.clone(),
            })
        }
    }

    pub(crate) fn opt_consume_semicolon(&mut self) -> Option<Token<'src>> {
        if let Some(t) = self.peek().cloned()
            && t.kind == TkKind::MarkSemiColon
        {
            self.next();
            Some(t.to_owned())
        } else {
            None
        }
    }

    pub(crate) fn consume_identifier(&mut self) -> Result<Ident, ParseError<'src>> {
        let t = self.next().ok_or(ParseError::InvalidEOF {
            expecteds: vec![TkKindName::Ident],
        })?;

        if let TkKind::Ident(id) = &t.kind {
            Ok(Ident {
                id: id.to_string(),
                span: t.span.clone(),
            })
        } else {
            Err(ParseError::InvalidToken {
                expecteds: vec![TkKindName::Ident],
                found: t.clone(),
            })
        }
    }

    pub(crate) fn consume_qualified_identifier(&mut self) -> Result<QualifiedId, ParseError<'src>> {
        let mut ids = vec![];
        let (is_from_root, begin, mut end) = if let Some(t) = self.peek().cloned()
            && matches!(t.kind, TkKind::KwPackage)
        {
            self.next();
            self.must_consume_next(vec![TkKindName::MarkDoubleColon])?;

            ids.push(self.consume_identifier()?.id);

            (true, t.span.clone(), t.span.clone())
        } else {
            let ident = self.consume_identifier()?;
            ids.push(ident.id);

            (false, ident.span.clone(), ident.span)
        };

        loop {
            if let Some(t) = self.peek() {
                if let TkKind::MarkDoubleColon = t.kind {
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
