use biwac_base::Span;
use biwac_lexer::TkKind;

use biwac_ast::{BlockExpr, Stmt};

use crate::{ExprOrStmt, ParseError, TokenStream, symbols::globals::FnParseCtx};

impl<'t> TokenStream<'t> {
    pub(crate) fn consume_block_expression(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<BlockExpr, ParseError> {
        let begin = self.must_consume_next(vec![TkKind::LBrace])?.span.clone();

        let mut stmts: Vec<Stmt> = vec![];

        loop {
            match self.consume_expression_or_statement(ctx)? {
                ExprOrStmt::Expr(expr) => {
                    let end = self.must_consume_next(vec![TkKind::RBrace])?.span.clone();

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
