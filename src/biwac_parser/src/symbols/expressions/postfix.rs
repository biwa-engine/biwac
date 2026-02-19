use biwac_lexer::TkKind;

use crate::{
    ParseError,
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

        self.consume_postfix_after_expression(expr)
    }

    fn consume_postfix_after_expression(&mut self, expr: Exprs) -> Result<Exprs, ParseError> {
        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Dot => {
                    self.next();

                    let mem_or_method = self.consume_identifier()?;

                    if let Some(t) = self.peek() {
                        if let TkKind::LPare = t.kind {
                            todo!()
                            // ISSUE: 何らかのデリミタを用意しないと、
                            // expr.method() と
                            // expr.member (anotherexpr)
                            // が区別できない
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
