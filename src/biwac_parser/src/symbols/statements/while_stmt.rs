use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        matches,
        symbols::{
            expressions::{self, Exprs},
            statements::block,
        },
        ParseError,
    },
};

use super::Stmt;

#[derive(Debug)]
pub struct WhileStmt {
    pub cond: Exprs,
    pub stmts: Vec<Stmt>,
}

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<WhileStmt, ParseError> {
    matches(tokens.next(), vec![TokenKind::While])?;

    let cond = expressions::consume(tokens)?;

    let stmts = block::consume(tokens)?;

    Ok(WhileStmt { cond, stmts })
}
