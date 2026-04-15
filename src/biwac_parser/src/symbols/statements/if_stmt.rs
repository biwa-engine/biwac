use biwac_base::Span;
use biwac_lexer::{TkKind, TkKindName};

use biwac_ast::{IfExpr, IfStmt};

use crate::{ExprOrStmt, ParseError, TokenStream, symbols::globals::FnParseCtx};

impl<'t, 'src> TokenStream<'t, 'src> {
    // "if" <expression> <block-statement> ("else" <block-statement>)?
    pub(super) fn consume_if_expression_or_statement(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<ExprOrStmt<IfExpr, IfStmt>, ParseError<'src>> {
        let begin = self.must_consume_next(vec![TkKindName::KwIf])?.span.clone();

        let cond = self.consume_expression(ctx)?;

        match self.consume_block_expression_or_statement(ctx)? {
            ExprOrStmt::Expr(then) => {
                self.must_consume_next(vec![TkKindName::KwElse])?;

                let els = self.consume_block_expression(ctx)?;

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

                    let els = self.consume_block_statement(ctx)?;

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
    pub(super) fn consume_if_statement(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<IfStmt, ParseError<'src>> {
        let begin = self.must_consume_next(vec![TkKindName::KwIf])?.span.clone();

        let cond = self.consume_expression(ctx)?;

        let then = self.consume_block_statement(ctx)?;

        if let Some(t) = self.peek()
            && let TkKind::KwElse = t.kind
        {
            self.next();

            let els = self.consume_block_statement(ctx)?;

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
