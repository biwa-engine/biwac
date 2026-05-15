use biwac_lexer::TkKind;

use biwac_ast::{BinOperator, BinaryExpr, Exprs};

use crate::{ParseError, TokenStream};

impl<'t, 'src> TokenStream<'t, 'src> {
    pub(super) fn consume_relational_expression(&mut self) -> Result<Exprs, ParseError<'src>> {
        let left = self.consume_arithmetic_expression()?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::MarkLesser => {
                    self.next();

                    // NOTE:
                    // relational expression は
                    // (Number ::= Int | Uint | Float と仮に置いたとき、)
                    // (Number, Number) -> Bool
                    // つまり、戻り値と引数の型が一致しないため、
                    // 再帰的に適用され得ない
                    // したがって、構文木レベルで再帰適用を弾いて良い
                    let right = self.consume_arithmetic_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Lt,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                TkKind::MarkGreater => {
                    self.next();

                    let right = self.consume_arithmetic_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Gt,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                TkKind::MarkLesEq => {
                    self.next();

                    let right = self.consume_arithmetic_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Le,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                TkKind::MarkGrtEq => {
                    self.next();

                    let right = self.consume_arithmetic_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Ge,
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
