use biwac_span::Span;

use biwac_ast::{Exprs, UnOperator, UnaryExpr};

use crate::{
    NCodeTokenOption, NovelParseError, NovelSourceStream,
    token::{NCodeTkKind, NCodeTkKindName},
};

impl<'src> NovelSourceStream<'src> {
    pub(super) fn consume_unary_expression(&mut self) -> Result<Exprs, NovelParseError> {
        match self.peek_token()? {
            NCodeTokenOption::Some(t) => match t.kind {
                NCodeTkKind::MarkMinus => {
                    let begin = t.span.clone();
                    self.next_token()?;

                    let expr = self.consume_unary_expression()?;
                    let span = Span::merge(&begin, &expr.span());

                    Ok(Exprs::Unary(UnaryExpr {
                        op: UnOperator::Neg,
                        right: Box::new(expr),
                        span,
                    }))
                }
                _ => self.consume_postfix_expression(),
            },
            NCodeTokenOption::None { idx } => Err(NovelParseError::InvalidLineEnd {
                expecteds: vec![NCodeTkKindName::Ident, NCodeTkKindName::MarkMinus],
                span: self.span_from(idx, 1),
            }),
        }
    }
}
