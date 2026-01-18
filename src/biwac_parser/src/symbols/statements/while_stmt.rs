use biwac_lexer::TkKind;

use crate::{Exprs, ParseError, Stmt, parser::TokenStream};

#[derive(Debug)]
pub struct WhileStmt {
    pub cond: Exprs,
    pub stmts: Vec<Stmt>,
}

impl<'t> TokenStream<'t> {
    pub fn consume_while_statement(&mut self) -> Result<WhileStmt, ParseError> {
        let _ = self.must_consume_next(vec![TkKind::While])?;
        let cond = self.consume_expression()?;
        let stmts = self.consume_block_statement()?;

        Ok(WhileStmt { cond, stmts })
    }
}
