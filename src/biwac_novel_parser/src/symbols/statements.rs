mod if_stmt;
mod vardecl;

use biwac_ast::{AssignStmt, ExprStmt, Exprs, NovelMessage, NovelStmt};
use biwac_base::Span;

use crate::{NovelLineKind, NovelParseError, NovelSourceStream, token::NCodeTkKind};

impl<'src> NovelSourceStream<'src> {
    pub(crate) fn consume_statement(&mut self) -> Result<Option<NovelStmt>, NovelParseError> {
        self.next_line()
            .map(|line_kind| {
                match line_kind {
                    NovelLineKind::RawNovel => {
                        // TODO:
                        // - 埋め込み式 $(expr) をパース
                        // - wait コマンド >> をパース

                        let line = self.lines.get(self.cursor.lidx).unwrap();

                        Ok(Some(NovelStmt::NovelWrite(NovelMessage {
                            msg: line.to_string(),
                            span: self.current_span(line.len()), // FIXME
                        })))
                    }
                    NovelLineKind::GeneralCommand => match self.peek_token()? {
                        Some(t) => match t.kind {
                            NCodeTkKind::KwIf => {
                                Ok(Some(NovelStmt::If(self.consume_if_statement()?)))
                            }
                            NCodeTkKind::KwLet => Ok(Some(NovelStmt::VarDecl(
                                self.consume_variable_declaration_statment()?,
                            ))),
                            // WARN: 意味のある式の実行(副作用のある関数の呼び出しなど)に限定するため、
                            // パーサの段階で
                            // - <identifier> 以外禁止とする
                            // - <function-calling> のパースを試みる
                            // としてもよい
                            _ => {
                                let expr = self.consume_expression()?;

                                if let Some(t) = self.peek_token()?.cloned()
                                    && let NCodeTkKind::MarkAssign = t.kind
                                {
                                    // <primary> "=" <expression>
                                    self.next_token()?;

                                    if let Exprs::Primary(dst) = expr {
                                        // <expression>
                                        let src = self.consume_expression()?;

                                        // <END_OF_LINE>
                                        self.must_be_line_end()?;

                                        Ok(Some(NovelStmt::Assign(AssignStmt {
                                            span: Span::merge(&dst.span(), &src.span()),
                                            dst,
                                            src,
                                        })))
                                    } else {
                                        Err(NovelParseError::LineEndExpected {
                                            found: Box::new(t.to_owned()),
                                        })
                                    }
                                } else {
                                    // <END_OF_LINE>
                                    self.must_be_line_end()?;

                                    Ok(Some(NovelStmt::Expr(ExprStmt {
                                        span: expr.span(),
                                        expr,
                                    })))
                                }
                            }
                        },

                        // # 以降に何もないとき
                        None => Err(NovelParseError::GeneralCommandLineOnlyPrefix {
                            span: self.current_span(1),
                        }),
                    },
                    NovelLineKind::CharaCommand => {
                        todo!()
                    }
                    NovelLineKind::BlockClose => {
                        // TODO: error
                        todo!()
                    }
                }
            })
            .transpose()
            .map(|opt| opt.flatten())
    }

    pub(crate) fn must_be_line_end(&mut self) -> Result<(), NovelParseError> {
        let t = self.next_token()?;

        match t {
            Some(t) => Err(NovelParseError::LineEndExpected { found: Box::new(t) }),
            None => Ok(()),
        }
    }
}
