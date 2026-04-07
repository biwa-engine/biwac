use biwac_ast::{BinOperator, BinaryExpr, Exprs};

use crate::{NovelParseError, NovelSourceStream, token::NCodeTkKind};

impl<'src> NovelSourceStream<'src> {
    pub(super) fn consume_equality_expression(&mut self) -> Result<Exprs, NovelParseError> {
        let left = self.consume_relational_expression()?;

        if let Some(t) = self.peek_token()? {
            match t.kind {
                NCodeTkKind::MarkEqual => {
                    self.next_token()?;

                    // NOTE:
                    // equality expression は
                    // (Bool, Bool) -> Bool
                    // つまり、戻り値と引数の型が一致しているため、
                    // 再帰的に適用され得る
                    let right = self.consume_equality_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Eq,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                NCodeTkKind::MarkNotEq => {
                    self.next_token()?;

                    let right = self.consume_equality_expression()?;

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
