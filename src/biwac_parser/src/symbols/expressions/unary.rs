use biwac_lexer::{TkKind, TkKindName};
use biwac_span::Span;

use biwac_ast::{Exprs, UnOperator, UnaryExpr};

use crate::{ParseError, TokenStream};

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    pub(super) fn consume_unary_expression(&mut self) -> Result<Exprs, ParseError<'src>> {
        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::MarkMinus => {
                    let begin = t.span.clone();
                    self.next();

                    let expr = self.consume_unary_expression()?;
                    let span = Span::merge(&begin, &expr.span());

                    Ok(Exprs::Unary(UnaryExpr {
                        op: UnOperator::Neg,
                        right: Box::new(expr),
                        span,
                    }))
                }
                _ => self.consume_postfix_expression(),
            }
        } else {
            Err(ParseError::InvalidEOF {
                mod_id: self.mod_id,
                expecteds: vec![TkKindName::Ident, TkKindName::MarkMinus],
            })
        }
    }
}
