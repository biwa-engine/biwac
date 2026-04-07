mod arithmetic;
mod equality;
mod multiplication;
mod postfix;
mod primary;
mod relational;
mod unary;

use biwac_ast::Exprs;

use crate::{NovelParseError, NovelSourceStream};

impl<'src> NovelSourceStream<'src> {
    pub(crate) fn consume_expression(&mut self) -> Result<Exprs, NovelParseError> {
        self.consume_equality_expression()
    }
}
