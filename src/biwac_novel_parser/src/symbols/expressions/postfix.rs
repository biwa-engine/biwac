use biwac_span::Span;

use biwac_ast::{Exprs, MemberAccess, MethodCall, Primary};

use crate::{NovelParseError, NovelSourceStream, token::NCodeTkKind};

impl<'src> NovelSourceStream<'src> {
    pub(super) fn consume_postfix_expression(&mut self) -> Result<Exprs, NovelParseError> {
        let expr = self.consume_primary_expression()?;

        self.consume_postfix_after_expression(expr)
    }

    fn consume_postfix_after_expression(&mut self, expr: Exprs) -> Result<Exprs, NovelParseError> {
        if let Some(t) = self.peek_token()? {
            match t.kind {
                NCodeTkKind::MarkDot => {
                    self.next_token()?;

                    let mem_or_method = self.consume_identifier()?;

                    if let Some(t) = self.peek_token()? {
                        if let NCodeTkKind::MarkLPare = t.kind {
                            let (args, span) = self.consume_arguments()?;

                            Ok(Exprs::Primary(Primary::MethodCall(MethodCall {
                                span: Span::merge(&expr.span(), &span),
                                left: Box::new(expr),
                                method: mem_or_method,
                                args,
                            })))
                        } else {
                            Ok(self.consume_postfix_after_expression(Exprs::Primary(
                                Primary::MemberAccess(MemberAccess {
                                    left: Box::new(expr),
                                    member: mem_or_method.clone(),
                                }),
                            ))?)
                        }
                    } else {
                        Ok(self.consume_postfix_after_expression(Exprs::Primary(
                            Primary::MemberAccess(MemberAccess {
                                left: Box::new(expr),
                                member: mem_or_method.clone(),
                            }),
                        ))?)
                    }
                }
                _ => Ok(expr),
            }
        } else {
            Ok(expr)
        }
    }
}
