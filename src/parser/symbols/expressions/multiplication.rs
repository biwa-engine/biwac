use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        symbols::expressions::{unary, BinOperator, Exprs},
        ParseError,
    },
};

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Exprs, ParseError> {
    let left = unary::consume(tokens)?;

    if let Some(t) = tokens.peek() {
        match t.kind {
            TokenKind::Asterisk => {
                tokens.next();

                let right = consume(tokens)?;

                Ok(Exprs::Binary(
                    BinOperator::Mul,
                    Box::new(left),
                    Box::new(right),
                ))
            }
            TokenKind::Slash => {
                tokens.next();

                let right = consume(tokens)?;

                Ok(Exprs::Binary(
                    BinOperator::Div,
                    Box::new(left),
                    Box::new(right),
                ))
            }
            TokenKind::Percent => {
                tokens.next();

                let right = consume(tokens)?;

                Ok(Exprs::Binary(
                    BinOperator::Mod,
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
