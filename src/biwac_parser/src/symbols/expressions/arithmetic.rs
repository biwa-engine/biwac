use biwac_lexer::TkKind;

use biwac_ast::{BinOperator, BinaryExpr, Exprs};

use crate::{ParseError, TokenStream};

impl<'t, 'src> TokenStream<'t, 'src> {
    pub(super) fn consume_arithmetic_expression(&mut self) -> Result<Exprs, ParseError<'src>> {
        let left = self.consume_multiplication_expression()?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::MarkPlus => {
                    self.next();

                    // NOTE:
                    // arithmetic expression は
                    // (Number ::= Int | Uint | Float と仮に置いたとき、)
                    // (Number, Number) -> Number
                    // つまり、戻り値と引数の型が一致しているため、
                    // 再帰的に適用され得る
                    let right = self.consume_arithmetic_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Add,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                TkKind::MarkMinus => {
                    self.next();

                    let right = self.consume_arithmetic_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Sub,
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
