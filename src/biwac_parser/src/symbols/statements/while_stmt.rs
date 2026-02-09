use biwac_base::Span;
use biwac_lexer::TkKind;

use crate::{BlockStmt, Exprs, ParseError, parser::TokenStream};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhileStmt {
    pub cond: Exprs,
    pub stmts: BlockStmt,
    pub span: Span,
}

impl<'t> TokenStream<'t> {
    // "while" <expression> <block-statement>
    pub fn consume_while_statement(&mut self) -> Result<WhileStmt, ParseError> {
        let begin = self.must_consume_next(vec![TkKind::While])?.span.clone();
        let cond = self.consume_expression()?;
        let stmts = self.consume_block_statement()?;

        Ok(WhileStmt {
            span: Span::merge(&begin, &stmts.span),
            cond,
            stmts,
        })
    }
}
