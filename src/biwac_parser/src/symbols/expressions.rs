pub mod arithmetic;
pub mod block;
pub mod equality;
pub mod multiplication;
pub mod postfix;
pub mod primary;
pub mod relational;
pub mod unary;

use biwac_ast::Exprs;

use crate::{ParseError, TokenStream};

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    pub(crate) fn consume_expression(&mut self) -> Result<Exprs, ParseError<'src>> {
        self.consume_equality_expression()
    }
}
