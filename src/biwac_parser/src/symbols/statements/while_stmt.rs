use biwac_base::Span;
use biwac_lexer::TkKind;

use biwac_ast::WhileStmt;

use crate::{ParseError, TokenStream, symbols::globals::FnParseCtx};

impl<'t> TokenStream<'t> {
    // "while" <expression> <block-statement>
    pub fn consume_while_statement(&mut self, ctx: &FnParseCtx) -> Result<WhileStmt, ParseError> {
        let begin = self.must_consume_next(vec![TkKind::While])?.span.clone();
        let cond = self.consume_expression(ctx)?;
        let stmts = self.consume_block_statement(ctx)?;

        Ok(WhileStmt {
            span: Span::merge(&begin, &stmts.span),
            cond,
            stmts,
        })
    }
}
