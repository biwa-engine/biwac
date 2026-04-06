use biwac_base::Span;
use biwac_lexer::TkKind;

use biwac_ast::{AssignStmt, BlockStmt, ExprStmt, Exprs, ReturnStmt, Stmt};

use crate::{ParseError, TokenStream, symbols::globals::FnParseCtx};

impl<'t> TokenStream<'t> {
    pub(crate) fn consume_block_statement(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<BlockStmt, ParseError> {
        let begin = self.must_consume_next(vec![TkKind::LBrace])?.span.clone();

        let mut stmts: Vec<Stmt> = vec![];

        loop {
            if let Some(t) = self.peek() {
                if let TkKind::RBrace = t.kind {
                    let end = t.span.clone();
                    self.next();
                    return Ok(BlockStmt {
                        stmts,
                        span: Span::merge(&begin, &end),
                    });
                } else {
                    let stmt = self.consume_statement(ctx)?;

                    stmts.push(stmt);
                }
            } else {
                return Err(ParseError::InvalidEOF(vec![TkKind::RBrace]));
            }
        }
    }

    pub(crate) fn consume_statement(&mut self, ctx: &FnParseCtx) -> Result<Stmt, ParseError> {
        if let Some(t) = self.peek().copied() {
            match t.kind {
                TkKind::If => Ok(Stmt::If(self.consume_if_statement(ctx)?)),
                TkKind::While => Ok(Stmt::While(self.consume_while_statement(ctx)?)),
                TkKind::Return => {
                    // "return" <expression> ";"
                    self.next();

                    // <expression>
                    let expr = self.consume_expression(ctx)?;

                    // ";"
                    let end = self.must_consume_semicolon()?.span.clone();

                    Ok(Stmt::Return(ReturnStmt {
                        expr,
                        span: Span::merge(&t.span, &end),
                    }))
                }
                TkKind::LBrace => Ok(Stmt::Block(self.consume_block_statement(ctx)?)),
                TkKind::Let => Ok(Stmt::VarDecl(
                    self.consume_variable_declaration_statment(Some(ctx))?,
                )),
                _ => {
                    let expr = self.consume_expression(ctx)?;

                    if let Some(t) = self.peek().copied()
                        && let TkKind::Assign = t.kind
                    {
                        // <primary> "=" <expression> ";"
                        self.next();

                        if let Exprs::Primary(dst) = expr {
                            // <expression>
                            let src = self.consume_expression(ctx)?;

                            // ";"
                            let end = self.must_consume_semicolon()?.span.clone();

                            Ok(Stmt::Assign(AssignStmt {
                                span: Span::merge(&dst.span(), &end),
                                dst,
                                src,
                            }))
                        } else {
                            Err(ParseError::InvalidToken(
                                vec![TkKind::SemiColon],
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
