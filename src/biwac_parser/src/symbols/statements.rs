pub mod block;
pub mod if_stmt;
pub mod vardec;
pub mod while_stmt;

use if_stmt::IfStmt;
use while_stmt::WhileStmt;

use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        symbols::{
            expressions::{self, Primary},
            statements::vardec::VarDec,
        },
        ParseError,
    },
};

use super::expressions::Exprs;

#[derive(Debug)]
pub enum Stmt {
    Block(Vec<Stmt>),
    Expr(Exprs),
    Return(Exprs),
    If(Box<IfStmt>),
    While(Box<WhileStmt>),
    VarDec(VarDec),
    Assign(Primary, Exprs), // dst, src
}

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Stmt, ParseError> {
    if let Some(t) = tokens.peek() {
        match t.kind {
            TokenKind::If => Ok(Stmt::If(Box::new(if_stmt::consume(tokens)?))),
            TokenKind::While => Ok(Stmt::While(Box::new(while_stmt::consume(tokens)?))),
            TokenKind::Return => {
                tokens.next();
                Ok(Stmt::Return(expressions::consume(tokens)?))
            }
            TokenKind::LBrace => Ok(Stmt::Block(block::consume(tokens)?)),
            TokenKind::Let => Ok(Stmt::VarDec(vardec::consume(tokens)?)),
            _ => {
                let expr = expressions::consume(tokens)?;

                if let Some(t) = tokens.peek().copied() {
                    if let TokenKind::Assign = t.kind {
                        tokens.next();

                        if let Exprs::Primary(dst) = expr {
                            let src = expressions::consume(tokens)?;

                            Ok(Stmt::Assign(dst, src))
                        } else {
                            Err(ParseError::InvalidToken(
                                vec![
                                    TokenKind::Let,
                                    TokenKind::If,
                                    TokenKind::While,
                                    TokenKind::Return,
                                ],
                                t.to_owned(),
                            ))
                        }
                    } else {
                        Ok(Stmt::Expr(expr))
                    }
                } else {
                    Ok(Stmt::Expr(expr))
                }
            }
        }
    } else {
        Err(ParseError::InvalidEOF(vec![
            TokenKind::Let,
            TokenKind::If,
            TokenKind::While,
            TokenKind::Return,
        ]))
    }
}
