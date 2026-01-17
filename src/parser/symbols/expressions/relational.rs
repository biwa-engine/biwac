use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        symbols::expressions::{arithmetic, BinOperator, Exprs},
        ParseError,
    },
};

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Exprs, ParseError> {
    let left = arithmetic::consume(tokens)?;

    if let Some(t) = tokens.peek() {
        match t.kind {
            TokenKind::Lesser => {
                tokens.next();

                let right = consume(tokens)?;

                Ok(Exprs::Binary(
                    BinOperator::Lt,
                    Box::new(left),
                    Box::new(right),
                ))
            }
            TokenKind::Greater => {
                tokens.next();

                let right = consume(tokens)?;

                Ok(Exprs::Binary(
                    BinOperator::Gt,
                    Box::new(left),
                    Box::new(right),
                ))
            }
            TokenKind::LesEq => {
                tokens.next();

                let right = consume(tokens)?;

                Ok(Exprs::Binary(
                    BinOperator::Le,
                    Box::new(left),
                    Box::new(right),
                ))
            }
            TokenKind::GrtEq => {
                tokens.next();

                let right = consume(tokens)?;

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
