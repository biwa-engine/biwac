pub mod block;
pub mod if_stmt;
pub mod vardecl;
pub mod while_stmt;

use biwac_base::Span;
use biwac_lexer::token::TkKind;
use if_stmt::IfStmt;
use while_stmt::WhileStmt;

use crate::{BlockStmt, ExprOrStmt, Exprs, ParseError, Primary, VarDecl, parser::TokenStream};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Block(BlockStmt),
    Expr(ExprStmt),
    Return(ReturnStmt),
    If(IfStmt),
    While(WhileStmt),
    VarDecl(VarDecl),
    Assign(AssignStmt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprStmt {
    pub expr: Exprs,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnStmt {
    pub expr: Exprs,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignStmt {
    pub dst: Primary,
    pub src: Exprs,
    pub span: Span,
}

impl<'t> TokenStream<'t> {
    // TODO: 将来的にはconsume_statement_or_expression
    // にして、呼び出す側でstatement/expressionそれぞれの場合のハンドリングをさせるべき
    pub(crate) fn consume_expression_or_statement(
        &mut self,
    ) -> Result<ExprOrStmt<Exprs, Stmt>, ParseError> {
        if let Some(t) = self.peek().copied() {
            match t.kind {
                TkKind::If => match self.consume_if_expression_or_statement()? {
                    ExprOrStmt::Expr(if_expr) => {
                        Ok(ExprOrStmt::Expr(Exprs::Primary(Primary::IfExpr(if_expr))))
                    }
                    ExprOrStmt::Stmt(if_stmt) => Ok(ExprOrStmt::Stmt(Stmt::If(if_stmt))),
                },
                TkKind::While => Ok(ExprOrStmt::Stmt(Stmt::While(
                    self.consume_while_statement()?,
                ))),
                TkKind::Return => {
                    // "return" <expression> ";"
                    self.next();

                    // <expression>
                    let expr = self.consume_expression()?;

                    // ";"
                    let end = self.must_consume_semicolon()?.span.clone();

                    Ok(ExprOrStmt::Stmt(Stmt::Return(ReturnStmt {
                        expr,
                        span: Span::merge(&t.span, &end),
                    })))
                }
                TkKind::LBrace => match self.consume_block_expression_or_statement()? {
                    ExprOrStmt::Expr(block_expr) => {
                        Ok(ExprOrStmt::Expr(Exprs::Primary(Primary::Block(block_expr))))
                    }
                    ExprOrStmt::Stmt(block_stmt) => Ok(ExprOrStmt::Stmt(Stmt::Block(block_stmt))),
                },
                TkKind::Let => Ok(ExprOrStmt::Stmt(Stmt::VarDecl(
                    self.consume_variable_declaration_statment()?,
                ))),
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

                            Ok(ExprOrStmt::Stmt(Stmt::Assign(AssignStmt {
                                span: Span::merge(&dst.span(), &end),
                                dst,
                                src,
                            })))
                        } else {
                            Err(ParseError::InvalidToken(
                                vec![TkKind::SemiColon],
                                t.to_owned(),
                            ))
                        }
                    } else {
                        // ";"
                        if let Some(t) = self.opt_consume_semicolon() {
                            // NOTE:
                            // セミコロンがあるならディスカードされて式文
                            // if there is a semicolon `;`, expression value is discarded,
                            // and it is treated as an expression-statement.
                            Ok(ExprOrStmt::Stmt(Stmt::Expr(ExprStmt {
                                span: Span::merge(&expr.span(), &t.span),
                                expr,
                            })))
                        } else {
                            // NOTE: ないなら、式
                            // if not, it is treated as an expression.
                            Ok(ExprOrStmt::Expr(expr))
                        }
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
