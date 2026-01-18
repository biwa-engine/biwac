use biwac_lexer::TkKind;

use crate::{
    ParseError,
    parser::TokenStream,
    symbols::expressions::{Exprs, UnOperator},
};

impl<'t> TokenStream<'t> {
    pub(super) fn consume_unary_expression(&mut self) -> Result<Exprs, ParseError> {
        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Minus => {
                    self.next();

                    let expr = self.consume_unary_expression()?;

                    Ok(Exprs::Unary(UnOperator::Neg, Box::new(expr)))
                }
                _ => self.consume_postfix_expression(),
            }
        } else {
            Err(ParseError::InvalidEOF(vec![TkKind::Ident, TkKind::Minus]))
        }
    }
}
