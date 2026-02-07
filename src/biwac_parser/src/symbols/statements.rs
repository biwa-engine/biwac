pub mod block;
pub mod if_stmt;
pub mod vardec;
pub mod while_stmt;

use biwac_base::Span;
use biwac_lexer::token::TkKind;
use if_stmt::IfStmt;
use while_stmt::WhileStmt;

use crate::{BlockStmt, Exprs, ParseError, Primary, VarDecl, parser::TokenStream};

#[derive(Debug, Clone)]
pub enum Stmt {
    Block(BlockStmt),
    Expr(ExprStmt),
    Return(ReturnStmt),
    If(IfStmt),
    While(WhileStmt),
    VarDecl(VarDecl),
    Assign(AssignStmt),
}

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Exprs,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub expr: Exprs,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AssignStmt {
    pub dst: Primary,
    pub src: Exprs,
    pub span: Span,
}

impl<'t> TokenStream<'t> {
    // TODO: 将来的にはconsume_statement_or_expression
    // にして、呼び出す側でstatement/expressionそれぞれの場合のハンドリングをさせるべき
    pub(crate) fn consume_statement(&mut self) -> Result<Stmt, ParseError> {
        if let Some(t) = self.peek().copied() {
            match t.kind {
                TkKind::If => Ok(Stmt::If(self.consume_if_statement()?)),
                TkKind::While => Ok(Stmt::While(self.consume_while_statement()?)),
                TkKind::Return => {
                    // "return" <expression> ";"
                    self.next();

                    // <expression>
                    let expr = self.consume_expression()?;

                    // ";"
                    let end = self.must_consume_semicolon()?.span.clone();

                    Ok(Stmt::Return(ReturnStmt {
                        expr,
                        span: Span::merge(&t.span, &end),
                    }))
                }
                TkKind::LBrace => Ok(Stmt::Block(self.consume_block_statement()?)),
                TkKind::Let => Ok(Stmt::VarDecl(self.consume_variable_declaration_statment()?)),
                _ => {
                    let expr = self.consume_expression()?;

                    if let Some(t) = self.peek().copied()
                        && let TkKind::Assign = t.kind
                    {
                        // <primary> "=" <expression> ";"
                        self.next();

                        if let Exprs::Primary(dst) = expr {
                            // <expression>
                            let src = self.consume_expression()?;

                            // ";"
                            let end = self.must_consume_semicolon()?.span.clone();

                            Ok(Stmt::Assign(AssignStmt {
                                span: Span::merge(&dst.span(), &end),
                                dst,
                                src,
                            }))
                        } else {
                            Err(ParseError::InvalidToken(
                                vec![TkKind::Let, TkKind::If, TkKind::While, TkKind::Return],
                                t.to_owned(),
                            ))
                        }
                    } else {
                        // ";"
                        let end = self.must_consume_semicolon()?.span.clone();

                        Ok(Stmt::Expr(ExprStmt {
                            span: Span::merge(&expr.span(), &end),
                            expr,
                        }))
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
