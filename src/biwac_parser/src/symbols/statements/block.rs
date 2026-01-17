use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        matches,
        symbols::statements::{self, Stmt},
        ParseError,
    },
};

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Vec<Stmt>, ParseError> {
    matches(tokens.next(), vec![TokenKind::LBrace])?;

    let mut stmts: Vec<Stmt> = vec![];

    loop {
        if let Some(t) = tokens.peek() {
            if let TokenKind::RBrace = t.kind {
                tokens.next();
                return Ok(stmts);
            }
        } else {
            return Err(ParseError::InvalidEOF(vec![TokenKind::RBrace]));
        }

        let stmt = statements::consume(tokens)?;

        stmts.push(stmt);
    }
}
