use biwac_base::Span;
use biwac_lexer::TkKind;

use crate::{BlockStmt, Exprs, ParseError, parser::TokenStream};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStmt {
    pub cond: Exprs,
    pub then: BlockStmt,
    pub els: Option<BlockStmt>,
    pub span: Span,
}

impl<'t> TokenStream<'t> {
    // "if" <expression> <block-statement> ("else" <block-statement>)?
    pub(super) fn consume_if_statement(&mut self) -> Result<IfStmt, ParseError> {
        let begin = self.must_consume_next(vec![TkKind::If])?.span.clone();

        let cond = self.consume_expression()?;

        let then = self.consume_block_statement()?;

        if let Some(t) = self.peek()
            && let TkKind::Else = t.kind
        {
            self.next();

            let els = self.consume_block_statement()?;

            Ok(IfStmt {
                span: Span::merge(&begin, &els.span),
                cond,
                then,
                els: Some(els),
            })
        } else {
            Ok(IfStmt {
                span: Span::merge(&begin, &then.span),
                cond,
                then,
                els: None,
            })
        }
    }
}
