use biwac_lexer::TkKind;

use crate::{ParseError, Stmt, parser::TokenStream};

impl<'t> TokenStream<'t> {
    pub(crate) fn consume_block_statement(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.must_consume_next(vec![TkKind::LBrace])?;

        let mut stmts: Vec<Stmt> = vec![];

        loop {
            if let Some(t) = self.peek() {
                if let TkKind::RBrace = t.kind {
                    self.next();
                    return Ok(stmts);
                }
            } else {
                return Err(ParseError::InvalidEOF(vec![TkKind::RBrace]));
            }

            let stmt = self.consume_statement()?;

            stmts.push(stmt);
        }
    }
}
