use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        symbols::expressions::{multiplication, BinOperator, Exprs},
        ParseError,
    },
};

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Exprs, ParseError> {
    let left = multiplication::consume(tokens)?;

    if let Some(t) = tokens.peek() {
        match t.kind {
            TokenKind::Plus => {
                tokens.next();

                let right = consume(tokens)?;

                Ok(Exprs::Binary(
                    BinOperator::Add,
                    Box::new(left),
                    Box::new(right),
                ))
            }
            TokenKind::Minus => {
                tokens.next();

                let right = consume(tokens)?;

                Ok(Exprs::Binary(
                    BinOperator::Sub,
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
