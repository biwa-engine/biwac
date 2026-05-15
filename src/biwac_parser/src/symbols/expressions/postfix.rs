use biwac_lexer::TkKind;
use biwac_span::Span;

use biwac_ast::{Exprs, MemberAccess, MethodCall, Primary};

use crate::{ParseError, TokenStream};

impl<'t, 'src> TokenStream<'t, 'src> {
    pub(super) fn consume_postfix_expression(&mut self) -> Result<Exprs, ParseError<'src>> {
        let expr = self.consume_primary_expression()?;

        self.consume_postfix_after_expression(expr)
    }

    fn consume_postfix_after_expression(&mut self, expr: Exprs) -> Result<Exprs, ParseError<'src>> {
        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::MarkDot => {
                    self.next();

                    let mem_or_method = self.consume_identifier()?;

                    if let Some(t) = self.peek() {
                        if let TkKind::MarkLPare = t.kind {
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
