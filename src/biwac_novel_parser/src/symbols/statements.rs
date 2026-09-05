mod end_scene;
mod if_stmt;
mod vardecl;

use biwac_ast::{AssignStmt, ExprStmt, Exprs, NovelMessage, NovelStmt, NovelWait};
use biwac_span::Span;

use crate::{
    NCodeTokenOption, NovelLineHandler, NovelLineKind, NovelLineOption, NovelParseError,
    NovelSourceStream, token::NCodeTkKind,
};

/// ノベルテキスト中の待ちコマンド。
const WAIT_COMMAND: &str = ">>";

pub(crate) enum ParsedNovelStmt {
    // novel mode の範囲の末尾に来ている場合は EndOfRange を返すため、
    // Stmts { stmts } は !stmts.is_empty() な Vec を常に返す
    Stmts { stmts: Vec<NovelStmt> },
    ExitBlock { line_handler: NovelLineHandler },
    EndOfRange { span: Span },
}

impl<'src> NovelSourceStream<'src> {
    pub(crate) fn consume_statements(&mut self) -> Result<ParsedNovelStmt, NovelParseError> {
        match self.next_line() {
            NovelLineOption::Some(line_handler) => {
                match line_handler.kind() {
                    NovelLineKind::RawNovel => {
                        // TODO:
                        // - 埋め込み式 $(expr) をパース

                        let line = self.line_str(&line_handler);
                        let span = self.line_span(&line_handler);

                        // wait コマンド `>>`。
                        //
                        // 行がこれだけなら待つだけ、
                        // 本文の後ろに付いていればその行を書いてから待つ。
                        match line.trim_end().strip_suffix(WAIT_COMMAND) {
                            Some(before) if before.trim().is_empty() => {
                                Ok(ParsedNovelStmt::Stmts {
                                    stmts: vec![NovelStmt::NovelWait(NovelWait {
                                        span: Span::new(
                                            span.module(),
                                            span.begin() + before.len(),
                                            span.end(),
                                        ),
                                    })],
                                })
                            }
                            Some(before) => {
                                // `>>` だけを取り除く。字下げと行末の改行は本文の一部として残す。
                                let cut = line.rfind(WAIT_COMMAND).expect("suffix was found");
                                let msg = format!(
                                    "{}{}",
                                    &line[..cut],
                                    &line[cut + WAIT_COMMAND.len()..]
                                );

                                Ok(ParsedNovelStmt::Stmts {
                                    stmts: vec![
                                        NovelStmt::NovelWrite(NovelMessage {
                                            msg,
                                            span: span.clone(),
                                        }),
                                        NovelStmt::NovelWait(NovelWait {
                                            span: Span::new(
                                                span.module(),
                                                span.begin() + before.len(),
                                                span.end(),
                                            ),
                                        }),
                                    ],
                                })
                            }
                            None => Ok(ParsedNovelStmt::Stmts {
                                stmts: vec![NovelStmt::NovelWrite(NovelMessage {
                                    msg: line.to_string(),
                                    span,
                                })],
                            }),
                        }
                    }
                    NovelLineKind::GeneralCommand => match self.peek_token()? {
                        NCodeTokenOption::Some(t) => match t.kind {
                            NCodeTkKind::KwIf => {
                                self.indent_enter();
                                Ok(ParsedNovelStmt::Stmts {
                                    stmts: vec![NovelStmt::If(self.consume_if_statement()?)],
                                })
                            }
                            NCodeTkKind::KwLet => Ok(ParsedNovelStmt::Stmts {
                                stmts: vec![NovelStmt::VarDecl(
                                    self.consume_variable_declaration_statment()?,
                                )],
                            }),
                            NCodeTkKind::KwEndScene => Ok(ParsedNovelStmt::Stmts {
                                stmts: vec![NovelStmt::NovelEndScene(
                                    self.consume_end_scene_statment()?,
                                )],
                            }),
                            // WARN: 意味のある式の実行(副作用のある関数の呼び出しなど)に限定するため、
                            // パーサの段階で
                            // - <identifier> 以外禁止とする
                            // - <function-calling> のパースを試みる
                            // としてもよい
                            _ => {
                                let expr = self.consume_expression()?;

                                if let NCodeTokenOption::Some(t) = self.peek_token()?.cloned()
                                    && let NCodeTkKind::MarkAssign = t.kind
                                {
                                    // <primary> "=" <expression>
                                    self.next_token()?;

                                    if let Exprs::Primary(dst) = expr {
                                        // <expression>
                                        let src = self.consume_expression()?;

                                        // <END_OF_LINE>
                                        self.must_be_line_end()?;

                                        Ok(ParsedNovelStmt::Stmts {
                                            stmts: vec![NovelStmt::Assign(AssignStmt {
                                                span: Span::merge(&dst.span(), &src.span()),
                                                dst,
                                                src,
                                            })],
                                        })
                                    } else {
                                        Err(NovelParseError::LineEndExpected {
                                            found: Box::new(t.to_owned()),
                                        })
                                    }
                                } else {
                                    // <END_OF_LINE>
                                    self.must_be_line_end()?;

                                    Ok(ParsedNovelStmt::Stmts {
                                        stmts: vec![NovelStmt::Expr(ExprStmt {
                                            span: expr.span(),
                                            expr,
                                        })],
                                    })
                                }
                            }
                        },

                        // # 以降に何もないとき
                        NCodeTokenOption::None { .. } => {
                            Err(NovelParseError::GeneralCommandLineOnlyPrefix {
                                span: self.line_span(&line_handler),
                            })
                        }
                    },
                    NovelLineKind::CharaCommand => {
                        todo!()
                    }

                    // } 行が来た場合、内側スコープの終了
                    NovelLineKind::BlockClose => {
                        self.indent_return();
                        Ok(ParsedNovelStmt::ExitBlock { line_handler })
                    }
                }
            }
            NovelLineOption::None { span } => Ok(ParsedNovelStmt::EndOfRange { span }),
        }
    }

    pub(crate) fn must_be_line_end(&mut self) -> Result<(), NovelParseError> {
        let t = self.next_token()?;

        match t {
            NCodeTokenOption::Some(t) => {
                Err(NovelParseError::LineEndExpected { found: Box::new(t) })
            }
            NCodeTokenOption::None { .. } => Ok(()),
        }
    }
}
