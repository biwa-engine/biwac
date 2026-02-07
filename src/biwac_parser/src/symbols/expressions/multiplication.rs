use biwac_lexer::TkKind;

use crate::{BinOperator, BinaryExpr, Exprs, ParseError, parser::TokenStream};

impl<'t> TokenStream<'t> {
    pub(super) fn consume_multiplication_expression(&mut self) -> Result<Exprs, ParseError> {
        let left = self.consume_unary_expression()?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Asterisk => {
                    self.next();

                    let right = self.consume_multiplication_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Mul,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                TkKind::Slash => {
                    self.next();

                    let right = self.consume_multiplication_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Div,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                TkKind::Percent => {
                    self.next();

                    let right = self.consume_multiplication_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Mod,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                _ => Ok(left),
            }
        } else {
            Ok(left)
        }
    }
}
