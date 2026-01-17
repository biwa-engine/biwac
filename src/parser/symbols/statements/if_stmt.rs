use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        matches,
        symbols::{
            expressions::{self, Exprs},
            statements::{
                block::{self},
                Stmt,
            },
        },
        ParseError,
    },
};

#[derive(Debug)]
pub struct IfStmt {
    pub cond: Exprs,
    pub then: Vec<Stmt>,
    pub els: Option<Vec<Stmt>>,
}

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<IfStmt, ParseError> {
    matches(tokens.next(), vec![TokenKind::If])?;
    let cond = expressions::consume(tokens)?;

    let then = block::consume(tokens)?;

    if let Some(t) = tokens.peek() {
        if let TokenKind::Else = t.kind {
            tokens.next();

            let els = block::consume(tokens)?;

            Ok(IfStmt {
                cond,
                then,
                els: Some(els),
            })
        } else {
            Ok(IfStmt {
                cond,
                then,
                els: None,
            })
        }
    } else {
        Ok(IfStmt {
            cond,
            then,
            els: None,
        })
    }
}
