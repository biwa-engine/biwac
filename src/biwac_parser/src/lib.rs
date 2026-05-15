mod error;
pub mod macros;
pub mod symbols;
pub mod types;

#[cfg(test)]
mod tests;

use biwac_ast::{AbsolutePathHeader, Ident, Path};
use biwac_base::{IdentInterner, ModId, ModPath};
use biwac_lexer::{TkKind, TkKindName, Token};

pub(crate) use symbols::statements::ExprOrStmt;

pub use error::ParseError;

#[derive(Debug)]
pub struct Parser<'src> {
    mod_id: ModId,
    modpath: ModPath,
    tokens: Vec<Token<'src>>,
    interner: &'src mut IdentInterner,
}

impl<'src> Parser<'src> {
    pub fn new(
        mod_id: ModId,
        modpath: ModPath,
        tokens: Vec<Token<'src>>,
        interner: &'src mut IdentInterner,
    ) -> Self {
        Self {
            mod_id,
            modpath,
            tokens,
            interner,
        }
    }
}

#[derive(Debug)]
pub(crate) struct TokenStream<'t, 'src> {
    mod_id: ModId,
    tokens: std::iter::Peekable<std::slice::Iter<'t, Token<'src>>>,
    interner: &'src mut IdentInterner,
}

impl<'t, 'src> TokenStream<'t, 'src> {
    pub(crate) fn new(
        mod_id: ModId,
        tokens: std::iter::Peekable<std::slice::Iter<'t, Token<'src>>>,
        interner: &'src mut IdentInterner,
    ) -> Self {
        Self {
            mod_id,
            tokens,
            interner,
        }
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
        let mod_id = self.mod_id;
        let t: &Token<'src> = self.next().ok_or(ParseError::InvalidEOF {
            mod_id,
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
        let mod_id = self.mod_id;
        let t = self
            .next()
            .ok_or(ParseError::InvalidEOF {
                mod_id,
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
        let mod_id = self.mod_id;
        let t = self.next().ok_or(ParseError::InvalidEOF {
            mod_id,
            expecteds: vec![TkKindName::Ident],
        })?;

        if let TkKind::Ident(id) = &t.kind {
            Ok(Ident {
                id: *id,
                span: t.span.clone(),
            })
        } else {
            Err(ParseError::InvalidToken {
                expecteds: vec![TkKindName::Ident],
                found: t.clone(),
            })
        }
    }

    pub(crate) fn consume_qualified_identifier(&mut self) -> Result<Path, ParseError<'src>> {
        let mut segments = vec![];
        let abs_header = if let Some(t) = self.peek().cloned()
            && matches!(t.kind, TkKind::KwPackage)
        {
            self.next();
            self.must_consume_next(vec![TkKindName::MarkDoubleColon])?;

            segments.push(self.consume_identifier()?.into());

            Some(AbsolutePathHeader::Package(t.span.clone()))
        } else {
            let ident = self.consume_identifier()?;
            segments.push(ident.into());

            None
        };

        loop {
            if let Some(t) = self.peek() {
                if let TkKind::MarkDoubleColon = t.kind {
                    self.next();
                    let ident = self.consume_identifier()?;
                    segments.push(ident.into());
                } else {
                    return Ok(Path::new(abs_header, segments));
                }
            } else {
                return Ok(Path::new(abs_header, segments));
            }
        }
    }
}
