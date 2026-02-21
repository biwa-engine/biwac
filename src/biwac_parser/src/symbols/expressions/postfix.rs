use biwac_base::Span;
use biwac_lexer::TkKind;

use crate::{
    MethodCall, ParseError,
    parser::TokenStream,
    symbols::{
        expressions::{Exprs, MemberAccess, Primary},
        globals::FnParseCtx,
    },
};

impl<'t> TokenStream<'t> {
    pub(super) fn consume_postfix_expression(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<Exprs, ParseError> {
        let expr = self.consume_primary_expression(ctx)?;

        self.consume_postfix_after_expression(expr, ctx)
    }

    fn consume_postfix_after_expression(
        &mut self,
        expr: Exprs,
        ctx: &FnParseCtx,
    ) -> Result<Exprs, ParseError> {
        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Dot => {
                    self.next();

                    let mem_or_method = self.consume_identifier()?;

                    if let Some(t) = self.peek() {
                        if let TkKind::LPare = t.kind {
                            let (args, span) = self.consume_arguments(ctx)?;

                            Ok(Exprs::Primary(Primary::MethodCall(MethodCall {
                                span: Span::merge(&expr.span(), &span),
                                left: Box::new(expr),
                                method: mem_or_method,
                                args,
                            })))
                        } else {
                            Ok(self.consume_postfix_after_expression(
                                Exprs::Primary(Primary::MemberAccess(MemberAccess {
                                    left: Box::new(expr),
                                    member: mem_or_method.clone(),
                                })),
                                ctx,
                            )?)
                        }
                    } else {
                        Ok(self.consume_postfix_after_expression(
                            Exprs::Primary(Primary::MemberAccess(MemberAccess {
                                left: Box::new(expr),
                                member: mem_or_method.clone(),
                            })),
                            ctx,
                        )?)
                    }
                }
                _ => Ok(expr),
            }
        } else {
            Ok(expr)
        }
    }
}
