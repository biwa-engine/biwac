use biwac_lexer::TkKind;

use crate::{BinOperator, Exprs, ParseError, parser::TokenStream};

impl<'t> TokenStream<'t> {
    pub(super) fn consume_relational_expression(&mut self) -> Result<Exprs, ParseError> {
        let left = self.consume_arithmetic_expression()?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Lesser => {
                    self.next();

                    // NOTE:
                    // relational expression は
                    // (Number ::= Int | Uint | Float と仮に置いたとき、)
                    // (Number, Number) -> Bool
                    // つまり、戻り値と引数の型が一致しないため、
                    // 再帰的に適用され得ない
                    // したがって、構文木レベルで再帰適用を弾いて良い
                    let right = self.consume_arithmetic_expression()?;

                    Ok(Exprs::Binary(
                        BinOperator::Lt,
                        Box::new(left),
                        Box::new(right),
                    ))
                }
                TkKind::Greater => {
                    self.next();

                    let right = self.consume_arithmetic_expression()?;

                    Ok(Exprs::Binary(
                        BinOperator::Gt,
                        Box::new(left),
                        Box::new(right),
                    ))
                }
                TkKind::LesEq => {
                    self.next();

                    let right = self.consume_arithmetic_expression()?;

                    Ok(Exprs::Binary(
                        BinOperator::Le,
                        Box::new(left),
                        Box::new(right),
                    ))
                }
                TkKind::GrtEq => {
                    self.next();

                    let right = self.consume_arithmetic_expression()?;

                    Ok(Exprs::Binary(
                        BinOperator::Ge,
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
