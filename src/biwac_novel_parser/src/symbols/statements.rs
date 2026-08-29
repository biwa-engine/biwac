mod end_scene;
mod if_stmt;
mod vardecl;

use biwac_ast::{AssignStmt, ExprStmt, Exprs, NovelMessage, NovelStmt, NovelWait};
use biwac_span::Span;

use crate::{NovelLineKind, NovelParseError, NovelSourceStream, token::NCodeTkKind};

/// ノベルテキスト中の待ちコマンド。
const WAIT_COMMAND: &str = ">>";

impl<'src> NovelSourceStream<'src> {
    pub(crate) fn consume_statements(&mut self) -> Result<Vec<NovelStmt>, NovelParseError> {
        self.next_line()
            .map(|line_kind| {
                match line_kind {
                    NovelLineKind::RawNovel => {
                        // TODO:
                        // - 埋め込み式 $(expr) をパース

                        let line = &self.src[self.line_begin_idx..self.next_line_begin_idx];
                        let span = self.current_span(line.len()); // FIXME

                        // wait コマンド `>>`。
                        //
                        // 行がこれだけなら待つだけ、
                        // 本文の後ろに付いていればその行を書いてから待つ。
                        match line.trim_end().strip_suffix(WAIT_COMMAND) {
                            Some(before) if before.trim().is_empty() => {
                                Ok(vec![NovelStmt::NovelWait(NovelWait { span })])
                            }
                            Some(_) => {
                                // `>>` だけを取り除く。字下げと行末の改行は本文の一部として残す。
                                let cut = line.rfind(WAIT_COMMAND).expect("suffix was found");
                                let msg = format!(
                                    "{}{}",
                                    &line[..cut],
                                    &line[cut + WAIT_COMMAND.len()..]
                                );

                                Ok(vec![
                                    NovelStmt::NovelWrite(NovelMessage {
                                        msg,
                                        span: span.clone(),
                                    }),
                                    NovelStmt::NovelWait(NovelWait { span }),
                                ])
                            }
                            None => Ok(vec![NovelStmt::NovelWrite(NovelMessage {
                                msg: line.to_string(),
                                span,
                            })]),
                        }
                    }
                    NovelLineKind::GeneralCommand => match self.peek_token()? {
                        Some(t) => match t.kind {
                            NCodeTkKind::KwIf => {
                                Ok(vec![NovelStmt::If(self.consume_if_statement()?)])
                            }
                            NCodeTkKind::KwLet => Ok(vec![NovelStmt::VarDecl(
                                self.consume_variable_declaration_statment()?,
                            )]),
                            NCodeTkKind::KwEndScene => Ok(vec![NovelStmt::NovelEndScene(
                                self.consume_end_scene_statment()?,
                            )]),
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

                                        Ok(vec![NovelStmt::Assign(AssignStmt {
                                            span: Span::merge(&dst.span(), &src.span()),
                                            dst,
                                            src,
                                        })])
                                    } else {
                                        Err(NovelParseError::LineEndExpected {
                                            found: Box::new(t.to_owned()),
                                        })
                                    }
                                } else {
                                    // <END_OF_LINE>
                                    self.must_be_line_end()?;

                                    Ok(vec![NovelStmt::Expr(ExprStmt {
                                        span: expr.span(),
                                        expr,
                                    })])
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

                    // } 行が予期せぬときに来た場合、
                    // 内側スコープの終了を考えて空で返す
                    NovelLineKind::BlockClose => Ok(Vec::new()),
                }
            })
            .transpose()
            .map(|opt_stmts| opt_stmts.into_iter().flatten().collect())
    }

    pub(crate) fn must_be_line_end(&mut self) -> Result<(), NovelParseError> {
        let t = self.next_token()?;

        match t {
            Some(t) => Err(NovelParseError::LineEndExpected { found: Box::new(t) }),
            None => Ok(()),
        }
    }
}
