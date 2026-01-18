use biwac_lexer::TkKind;

use crate::{BinOperator, Exprs, ParseError, parser::TokenStream};

impl<'t> TokenStream<'t> {
    pub(super) fn consume_multiplication_expression(&mut self) -> Result<Exprs, ParseError> {
        let left = self.consume_unary_expression()?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Asterisk => {
                    self.next();

                    let right = self.consume_multiplication_expression()?;

                    Ok(Exprs::Binary(
                        BinOperator::Mul,
                        Box::new(left),
                        Box::new(right),
                    ))
                }
                TkKind::Slash => {
                    self.next();

                    let right = self.consume_multiplication_expression()?;

                    Ok(Exprs::Binary(
                        BinOperator::Div,
                        Box::new(left),
                        Box::new(right),
                    ))
                }
                TkKind::Percent => {
                    self.next();

                    let right = self.consume_multiplication_expression()?;

                    Ok(Exprs::Binary(
                        BinOperator::Mod,
                        Box::new(left),
                        Box::new(right),
                    ))
                }
                _ => Ok(left),
            }
        } else {
            Ok(left)
        }
    }
}
