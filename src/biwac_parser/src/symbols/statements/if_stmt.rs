use biwac_lexer::TkKind;

use crate::{Exprs, ParseError, Stmt, parser::TokenStream};

#[derive(Debug)]
pub struct IfStmt {
    pub cond: Exprs,
    pub then: Vec<Stmt>,
    pub els: Option<Vec<Stmt>>,
}

impl<'t> TokenStream<'t> {
    pub(super) fn consume_if_statement(&mut self) -> Result<IfStmt, ParseError> {
        self.must_consume_next(vec![TkKind::If])?;

        let cond = self.consume_expression()?;

        let then = self.consume_block_statement()?;

        if let Some(t) = self.peek() {
            if let TkKind::Else = t.kind {
                self.next();

                let els = self.consume_block_statement()?;

                Ok(IfStmt {
                    cond,
                    then,
                    els: Some(els),
                })
            } else {
                Ok(IfStmt {
                    cond,
                    then,
                    els: None,
                })
            }
        } else {
            Ok(IfStmt {
                cond,
                then,
                els: None,
            })
        }
    }
}
