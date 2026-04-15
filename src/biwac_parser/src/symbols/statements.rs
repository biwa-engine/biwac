pub mod block;
pub mod if_stmt;
pub mod vardecl;
pub mod while_stmt;

use biwac_base::Span;
use biwac_lexer::{TkKindName, token::TkKind};

use biwac_ast::{AssignStmt, BlockExpr, BlockStmt, ExprStmt, Exprs, Primary, ReturnStmt, Stmt};

use crate::{ParseError, TokenStream, symbols::globals::FnParseCtx};

// パースすると判明する
// statement か expression を保持する
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExprOrStmt<E, S> {
    Expr(E),
    Stmt(S),
}

impl<'t, 'src> TokenStream<'t, 'src> {
    // TODO: 将来的にはconsume_statement_or_expression
    // にして、呼び出す側でstatement/expressionそれぞれの場合のハンドリングをさせるべき
    pub(crate) fn consume_expression_or_statement(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<ExprOrStmt<Exprs, Stmt>, ParseError<'src>> {
        if let Some(t) = self.peek().copied() {
            match t.kind {
                TkKind::KwIf => match self.consume_if_expression_or_statement(ctx)? {
                    ExprOrStmt::Expr(if_expr) => {
                        Ok(ExprOrStmt::Expr(Exprs::Primary(Primary::IfExpr(if_expr))))
                    }
                    ExprOrStmt::Stmt(if_stmt) => Ok(ExprOrStmt::Stmt(Stmt::If(if_stmt))),
                },
                TkKind::KwWhile => Ok(ExprOrStmt::Stmt(Stmt::While(
                    self.consume_while_statement(ctx)?,
                ))),
                TkKind::KwReturn => {
                    // "return" <expression> ";"
                    self.next();

                    // <expression>
                    let expr = self.consume_expression(ctx)?;

                    // ";"
                    let end = self.must_consume_semicolon()?.span.clone();

                    Ok(ExprOrStmt::Stmt(Stmt::Return(ReturnStmt {
                        expr,
                        span: Span::merge(&t.span, &end),
                    })))
                }
                TkKind::MarkLBrace => match self.consume_block_expression_or_statement(ctx)? {
                    ExprOrStmt::Expr(block_expr) => {
                        Ok(ExprOrStmt::Expr(Exprs::Primary(Primary::Block(block_expr))))
                    }
                    ExprOrStmt::Stmt(block_stmt) => Ok(ExprOrStmt::Stmt(Stmt::Block(block_stmt))),
                },
                TkKind::KwLet => Ok(ExprOrStmt::Stmt(Stmt::VarDecl(
                    self.consume_variable_declaration_statment(Some(ctx))?,
                ))),
                _ => {
                    let expr = self.consume_expression(ctx)?;

                    if let Some(t) = self.peek().copied()
                        && let TkKind::MarkAssign = t.kind
                    {
                        // <primary> "=" <expression> ";"
                        self.next();

                        if let Exprs::Primary(dst) = expr {
                            // <expression>
                            let src = self.consume_expression(ctx)?;

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
        ctx: &FnParseCtx,
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
                match self.consume_expression_or_statement(ctx)? {
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
