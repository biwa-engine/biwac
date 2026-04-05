use crate::{NovelLineKind, NovelParseError, NovelSourceStream, token::NCodeTkKind};

#[derive(Debug, Clone)]
pub enum NStmt {
    IfStmt,
    Expr,
}

impl<'src> NovelSourceStream<'src> {
    pub(crate) fn parse_statement(&mut self) -> Result<Option<NStmt>, NovelParseError<'src>> {
        self.next_line()
            .map(|line_kind| {
                match line_kind {
                    NovelLineKind::RawNovel => {
                        // TODO: ノベル
                    }
                    NovelLineKind::GeneralCommand => {
                        match self.next_token()? {
                            Some(t) => match t.kind {
                                NCodeTkKind::KwIf => {
                                    todo!()
                                }
                                _ => {
                                    todo!()
                                }
                            },

                            // # 以降に何もないとき
                            None => {
                                // error
                                todo!()
                            }
                        }
                    }
                    NovelLineKind::CharaCommand => {
                        todo!()
                    }
                    NovelLineKind::BlockClose => {
                        // TODO: 残りは空白文字のみであることを検査
                    }
                }

                todo!()
            })
            .transpose()
    }

    // fn parse_if_statement(&mut self, )
}
