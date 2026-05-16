use biwac_lexer::TkKindName;
use biwac_span::Span;

use biwac_ast::{BlockExpr, Stmt};

use crate::{ExprOrStmt, ParseError, TokenStream};

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    pub(crate) fn consume_block_expression(&mut self) -> Result<BlockExpr, ParseError<'src>> {
        let begin = self
            .must_consume_next(vec![TkKindName::MarkLBrace])?
            .span
            .clone();

        let mut stmts: Vec<Stmt> = vec![];

        loop {
            match self.consume_expression_or_statement()? {
                ExprOrStmt::Expr(expr) => {
                    let end = self
                        .must_consume_next(vec![TkKindName::MarkRBrace])?
                        .span
                        .clone();

                    return Ok(BlockExpr {
                        stmts,
                        expr: Box::new(expr),
                        span: Span::merge(&begin, &end),
                    });
                }
                ExprOrStmt::Stmt(stmt) => {
                    stmts.push(stmt);
                }
            }
        }
    }
}
