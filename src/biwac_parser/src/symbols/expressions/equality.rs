use biwac_lexer::TkKind;

use crate::{BinOperator, Exprs, ParseError, parser::TokenStream};

impl<'t> TokenStream<'t> {
    pub(super) fn consume_equality_expression(&mut self) -> Result<Exprs, ParseError> {
        let left = self.consume_relational_expression()?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Equal => {
                    self.next();

                    // NOTE:
                    // equality expression は
                    // (Bool, Bool) -> Bool
                    // つまり、戻り値と引数の型が一致しているため、
                    // 再帰的に適用され得る
                    let right = self.consume_equality_expression()?;

                    Ok(Exprs::Binary(
                        BinOperator::Eq,
                        Box::new(left),
                        Box::new(right),
                    ))
                }
                TkKind::NotEq => {
                    self.next();

                    let right = self.consume_equality_expression()?;

                    Ok(Exprs::Binary(
                        BinOperator::Ne,
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
