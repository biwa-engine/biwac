mod if_stmt;
mod vardecl;

use biwac_ast::{AssignStmt, ExprStmt, Exprs, Stmt};
use biwac_base::Span;

use crate::{NovelLineKind, NovelParseError, NovelSourceStream, token::NCodeTkKind};

impl<'src> NovelSourceStream<'src> {
    pub(crate) fn consume_statement(&mut self) -> Result<Option<Stmt>, NovelParseError<'src>> {
        self.next_line()
            .map(|line_kind| {
                match line_kind {
                    NovelLineKind::RawNovel => {
                        // TODO: ノベル
                    }
                    NovelLineKind::GeneralCommand => {
                        match self.peek_token()? {
                            Some(t) => match t.kind {
                                NCodeTkKind::KwIf => {
                                    Ok(Some(Stmt::If(self.consume_if_statement()?)))
                                }
                                NCodeTkKind::KwLet => Ok(Some(Stmt::VarDecl(
                                    self.consume_variable_declaration_statment()?,
                                ))),
                                // WARN: 意味のある式の実行(副作用のある関数の呼び出しなど)に限定するため、
                                // パーサの段階で
                                // - <identifier> 以外禁止とする
                                // - <function-calling> のパースを試みる
                                // としてもよい
                                _ => {
                                    let expr = self.consume_expression()?;

                                    if let Some(t) = self.peek_token()?.clone()
                                        && let NCodeTkKind::MarkAssign = t.kind
                                    {
                                        // <primary> "=" <expression>
                                        self.next_token()?;

                                        if let Exprs::Primary(dst) = expr {
                                            // <expression>
                                            let src = self.consume_expression()?;

                                            // <END_OF_LINE>
                                            self.must_be_line_end()?;

                                            Ok(Stmt::Assign(AssignStmt {
                                                span: Span::merge(&dst.span(), &src.span()),
                                                dst,
                                                src,
                                            }))
                                        } else {
                                            Err(NovelParseError::LineEndExpected {
                                                found: Box::new(t.to_owned()),
                                            })
                                        }
                                    } else {
                                        // <END_OF_LINE>
                                        self.must_be_line_end()?;

                                        Ok(Stmt::Expr(ExprStmt {
                                            span: expr.span(),
                                            expr,
                                        }))
                                    }
                                }
                            },

                            // # 以降に何もないとき
                            None => Err(NovelParseError::GeneralCommandLineOnlyPrefix {
                                span: self.current_span(1),
                            }),
                        }
                    }
                    NovelLineKind::CharaCommand => {
                        todo!()
                    }
                    NovelLineKind::BlockClose => {
                        // TODO: 残りは空白文字のみであることを検査
                        todo!()
                    }
                }
            })
            .transpose()
    }

    pub(crate) fn must_be_line_end(&mut self) -> Result<(), NovelParseError> {
        let t = self.next_token()?;

        match t {
            Some(t) => Err(NovelParseError::LineEndExpected { found: Box::new(t) }),
            None => Ok(()),
        }
    }
}
