use biwac_base::Span;
use biwac_lexer::TkKind;

use biwac_ast::{Exprs, UnOperator, UnaryExpr};

use crate::{ParseError, TokenStream, symbols::globals::FnParseCtx};

impl<'t> TokenStream<'t> {
    pub(super) fn consume_unary_expression(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<Exprs, ParseError> {
        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Minus => {
                    let begin = t.span.clone();
                    self.next();

                    let expr = self.consume_unary_expression(ctx)?;
                    let span = Span::merge(&begin, &expr.span());

                    Ok(Exprs::Unary(UnaryExpr {
                        op: UnOperator::Neg,
                        right: Box::new(expr),
                        span,
                    }))
                }
                _ => self.consume_postfix_expression(ctx),
            }
        } else {
            Err(ParseError::InvalidEOF(vec![TkKind::Ident, TkKind::Minus]))
        }
    }
}
