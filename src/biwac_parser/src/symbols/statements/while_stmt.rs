use biwac_lexer::TkKindName;
use biwac_span::Span;

use biwac_ast::WhileStmt;

use crate::{ParseError, TokenStream};

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    // "while" <expression> <block-statement>
    pub fn consume_while_statement(&mut self) -> Result<WhileStmt, ParseError<'src>> {
        let begin = self
            .must_consume_next(vec![TkKindName::KwWhile])?
            .span
            .clone();
        let cond = self.consume_condition_expression()?;
        let stmts = self.consume_block_statement()?;

        Ok(WhileStmt {
            span: Span::merge(&begin, &stmts.span),
            cond,
            stmts,
        })
    }
}
