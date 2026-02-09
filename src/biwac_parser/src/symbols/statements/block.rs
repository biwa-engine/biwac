use biwac_base::Span;
use biwac_lexer::TkKind;

use crate::{ParseError, Stmt, parser::TokenStream};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStmt {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

impl<'t> TokenStream<'t> {
    pub(crate) fn consume_block_statement(&mut self) -> Result<BlockStmt, ParseError> {
        let begin = self.must_consume_next(vec![TkKind::LBrace])?.span.clone();

        let mut stmts: Vec<Stmt> = vec![];

        loop {
            if let Some(t) = self.peek() {
                if let TkKind::RBrace = t.kind {
                    let end = t.span.clone();
                    self.next();
                    return Ok(BlockStmt {
                        stmts,
                        span: Span::merge(&begin, &end),
                    });
                } else {
                    let stmt = self.consume_statement()?;

                    stmts.push(stmt);
                }
            } else {
                return Err(ParseError::InvalidEOF(vec![TkKind::RBrace]));
            }
        }
    }
}
