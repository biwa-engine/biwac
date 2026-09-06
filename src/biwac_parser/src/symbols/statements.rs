pub mod block;
pub mod if_stmt;
pub mod match_stmt;
pub mod vardecl;
pub mod while_stmt;

use biwac_lexer::{TkKindName, token::TkKind};
use biwac_span::Span;

use biwac_ast::{AssignStmt, BlockExpr, BlockStmt, ExprStmt, Exprs, Primary, ReturnStmt, Stmt};

use crate::{ParseError, TokenStream};

// パースすると判明する
// statement か expression を保持する
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExprOrStmt<E, S> {
    Expr(E),
    Stmt(S),
}

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    pub(crate) fn consume_expression_or_statement(
        &mut self,
    ) -> Result<ExprOrStmt<Exprs, Stmt>, ParseError<'src>> {
        if let Some(t) = self.peek().copied() {
            match t.kind {
                TkKind::KwIf => match self.consume_if_expression_or_statement()? {
                    ExprOrStmt::Expr(if_expr) => {
                        Ok(ExprOrStmt::Expr(Exprs::Primary(Primary::IfExpr(if_expr))))
                    }
                    ExprOrStmt::Stmt(if_stmt) => Ok(ExprOrStmt::Stmt(Stmt::If(if_stmt))),
                },
                TkKind::KwMatch => match self.consume_match_expression_or_statement()? {
                    ExprOrStmt::Expr(m) => Ok(ExprOrStmt::Expr(Exprs::Primary(Primary::Match(m)))),
                    ExprOrStmt::Stmt(m) => Ok(ExprOrStmt::Stmt(Stmt::Match(m))),
                },
                TkKind::KwWhile => Ok(ExprOrStmt::Stmt(Stmt::While(
                    self.consume_while_statement()?,
                ))),
                TkKind::KwReturn => {
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
                TkKind::MarkLBrace => match self.consume_block_expression_or_statement()? {
                    ExprOrStmt::Expr(block_expr) => {
                        Ok(ExprOrStmt::Expr(Exprs::Primary(Primary::Block(block_expr))))
                    }
                    ExprOrStmt::Stmt(block_stmt) => Ok(ExprOrStmt::Stmt(Stmt::Block(block_stmt))),
                },
                TkKind::KwLet => Ok(ExprOrStmt::Stmt(Stmt::VarDecl(
                    self.consume_variable_declaration_statment()?,
                ))),
                _ => {
                    let expr = self.consume_expression()?;

                    if let Some(t) = self.peek().copied()
                        && let TkKind::MarkAssign = t.kind
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
                            Err(ParseError::InvalidToken {
                                expecteds: vec![TkKindName::MarkSemiColon],
                                found: t.to_owned(),
                            })
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
            Err(ParseError::InvalidEOF {
                mod_id: self.mod_id,
                expecteds: vec![
                    TkKindName::KwLet,
                    TkKindName::KwIf,
                    TkKindName::KwWhile,
                    TkKindName::KwReturn,
                ],
            })
        }
    }

    pub(crate) fn consume_block_expression_or_statement(
        &mut self,
    ) -> Result<ExprOrStmt<BlockExpr, BlockStmt>, ParseError<'src>> {
        let begin = self
            .must_consume_next(vec![TkKindName::MarkLBrace])?
            .span
            .clone();

        let mut stmts: Vec<Stmt> = vec![];

        loop {
            if let Some(t) = self.peek().copied()
                && TkKind::MarkRBrace == t.kind
            {
                self.next();

                return Ok(ExprOrStmt::Stmt(BlockStmt {
                    stmts,
                    span: Span::merge(&begin, &t.span),
                }));
            } else {
                match self.consume_expression_or_statement()? {
                    ExprOrStmt::Expr(expr) => {
                        let end = self
                            .must_consume_next(vec![TkKindName::MarkRBrace])?
                            .span
                            .clone();

                        return Ok(ExprOrStmt::Expr(BlockExpr {
                            stmts,
                            expr: Box::new(expr),
                            span: Span::merge(&begin, &end),
                        }));
                    }
                    ExprOrStmt::Stmt(stmt) => {
                        stmts.push(stmt);
                    }
                }
            }
        }
    }
}
