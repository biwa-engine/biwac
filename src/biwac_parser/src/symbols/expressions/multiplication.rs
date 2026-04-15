use biwac_lexer::TkKind;

use biwac_ast::{BinOperator, BinaryExpr, Exprs};

use crate::{ParseError, TokenStream, symbols::globals::FnParseCtx};

impl<'t, 'src> TokenStream<'t, 'src> {
    pub(super) fn consume_multiplication_expression(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<Exprs, ParseError<'src>> {
        let left = self.consume_unary_expression(ctx)?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::MarkAsterisk => {
                    self.next();

                    let right = self.consume_multiplication_expression(ctx)?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Mul,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                TkKind::MarkSlash => {
                    self.next();

                    let right = self.consume_multiplication_expression(ctx)?;

                    Ok(Exprs::Binary(BinaryExpr {
                        op: BinOperator::Div,
                        left: Box::new(left),
                        right: Box::new(right),
                    }))
                }
                TkKind::MarkPercent => {
                    self.next();

                    let right = self.consume_multiplication_expression(ctx)?;

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
