use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        symbols::expressions::{postfix, Exprs, UnOperator},
        ParseError,
    },
};

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Exprs, ParseError> {
    if let Some(t) = tokens.peek() {
        match t.kind {
            TokenKind::Minus => {
                tokens.next();

                let expr = consume(tokens)?;

                Ok(Exprs::Unary(UnOperator::Neg, Box::new(expr)))
            }
            _ => postfix::consume(tokens),
        }
    } else {
        Err(ParseError::InvalidEOF(vec![
            TokenKind::Identifier("".to_string()),
            TokenKind::Minus,
        ]))
    }
}
