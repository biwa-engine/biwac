pub mod arithmetic;
pub mod block;
pub mod equality;
pub mod multiplication;
pub mod postfix;
pub mod primary;
pub mod relational;
pub mod unary;

use biwac_ast::Exprs;

use crate::{ParseError, TokenStream, symbols::globals::FnParseCtx};

impl<'t, 'src> TokenStream<'t, 'src> {
    pub(crate) fn consume_expression(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<Exprs, ParseError<'src>> {
        self.consume_equality_expression(ctx)
    }
}
