use biwac_lexer::TkKind;

use biwac_ast::{BinOperator, BinaryExpr, Exprs};

use crate::{ParseError, TokenStream, symbols::globals::FnParseCtx};

impl<'t> TokenStream<'t> {
    pub(super) fn consume_equality_expression(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<Exprs, ParseError> {
        let left = self.consume_relational_expression(ctx)?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Equal => {
                    self.next();

                    // NOTE:
                    // equality expression は
                    // (Bool, Bool) -> Bool
                    // つまり、戻り値と引数の型が一致しているため、
                    // 再帰的に適用され得る
                    let right = self.consume_equality_expression(ctx)?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Eq,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                TkKind::NotEq => {
                    self.next();

                    let right = self.consume_equality_expression(ctx)?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Ne,
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
