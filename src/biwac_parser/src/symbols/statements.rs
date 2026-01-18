pub mod block;
pub mod if_stmt;
pub mod vardec;
pub mod while_stmt;

use biwac_lexer::token::TkKind;
use if_stmt::IfStmt;
use while_stmt::WhileStmt;

use crate::{Exprs, ParseError, VarDec, parser::TokenStream, symbols::expressions::Primary};

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

impl<'t> TokenStream<'t> {
    pub(crate) fn consume_statement(&mut self) -> Result<Stmt, ParseError> {
        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::If => Ok(Stmt::If(Box::new(self.consume_if_statement()?))),
                TkKind::While => Ok(Stmt::While(Box::new(self.consume_while_statement()?))),
                TkKind::Return => {
                    self.next();
                    Ok(Stmt::Return(self.consume_expression()?))
                }
                TkKind::LBrace => Ok(Stmt::Block(self.consume_block_statement()?)),
                TkKind::Let => Ok(Stmt::VarDec(self.consume_variable_declaration_statment()?)),
                _ => {
                    let expr = self.consume_expression()?;

                    if let Some(t) = self.peek().copied() {
                        if let TkKind::Assign = t.kind {
                            self.next();

                            if let Exprs::Primary(dst) = expr {
                                let src = self.consume_expression()?;

                                Ok(Stmt::Assign(dst, src))
                            } else {
                                Err(ParseError::InvalidToken(
                                    vec![TkKind::Let, TkKind::If, TkKind::While, TkKind::Return],
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
                TkKind::Let,
                TkKind::If,
                TkKind::While,
                TkKind::Return,
            ]))
        }
    }
}
