use biwac_lexer::{TkKind, TkKindName};
use biwac_span::Span;

use biwac_ast::{IfExpr, IfStmt};

use crate::{ExprOrStmt, ParseError, TokenStream};

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    // "if" <expression> <block-statement> ("else" <block-statement>)?
    pub(super) fn consume_if_expression_or_statement(
        &mut self,
    ) -> Result<ExprOrStmt<IfExpr, IfStmt>, ParseError<'src>> {
        let begin = self.must_consume_next(vec![TkKindName::KwIf])?.span.clone();

        let cond = self.consume_expression()?;

        match self.consume_block_expression_or_statement()? {
            ExprOrStmt::Expr(then) => {
                self.must_consume_next(vec![TkKindName::KwElse])?;

                let els = self.consume_block_expression()?;

                Ok(ExprOrStmt::Expr(IfExpr {
                    span: Span::merge(&begin, &els.span),
                    cond: Box::new(cond),
                    then,
                    els,
                }))
            }
            ExprOrStmt::Stmt(then) => {
                if let Some(t) = self.peek()
                    && let TkKind::KwElse = t.kind
                {
                    self.next();

                    let els = self.consume_block_statement()?;

                    Ok(ExprOrStmt::Stmt(IfStmt {
                        span: Span::merge(&begin, &els.span),
                        cond,
                        then,
                        els: Some(els),
                    }))
                } else {
                    Ok(ExprOrStmt::Stmt(IfStmt {
                        span: Span::merge(&begin, &then.span),
                        cond,
                        then,
                        els: None,
                    }))
                }
            }
        }
    }

    // "if" <expression> <block-statement> ("else" <block-statement>)?
    pub(super) fn consume_if_statement(&mut self) -> Result<IfStmt, ParseError<'src>> {
        let begin = self.must_consume_next(vec![TkKindName::KwIf])?.span.clone();

        let cond = self.consume_expression()?;

        let then = self.consume_block_statement()?;

        if let Some(t) = self.peek()
            && let TkKind::KwElse = t.kind
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
