use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        symbols::expressions::{relational, BinOperator, Exprs},
        ParseError,
    },
};

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Exprs, ParseError> {
    let left = relational::consume(tokens)?;

    if let Some(t) = tokens.peek() {
        match t.kind {
            TokenKind::Equal => {
                tokens.next();

                let right = consume(tokens)?;

                Ok(Exprs::Binary(
                    BinOperator::Eq,
                    Box::new(left),
                    Box::new(right),
                ))
            }
            TokenKind::NotEq => {
                tokens.next();

                let right = consume(tokens)?;

                Ok(Exprs::Binary(
                    BinOperator::Ne,
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
