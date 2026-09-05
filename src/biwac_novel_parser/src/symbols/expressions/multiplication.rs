use biwac_ast::{BinOperator, BinaryExpr, Exprs};

use crate::{NCodeTokenOption, NovelParseError, NovelSourceStream, token::NCodeTkKind};

impl<'src> NovelSourceStream<'src> {
    pub(super) fn consume_multiplication_expression(&mut self) -> Result<Exprs, NovelParseError> {
        let left = self.consume_unary_expression()?;

        if let NCodeTokenOption::Some(t) = self.peek_token()? {
            match t.kind {
                NCodeTkKind::MarkAsterisk => {
                    self.next_token()?;

                    let right = self.consume_multiplication_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Mul,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                NCodeTkKind::MarkSlash => {
                    self.next_token()?;

                    let right = self.consume_multiplication_expression()?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Div,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                NCodeTkKind::MarkPercent => {
                    self.next_token()?;

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
