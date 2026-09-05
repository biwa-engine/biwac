use biwac_span::Span;

use crate::NovelSourceStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NovelLineKind {
    RawNovel,       // 生ノベルテキスト
    GeneralCommand, // # 行
    CharaCommand,   // @ 行
    BlockClose,     // } 行
}

#[derive(Debug, Clone)]
pub(crate) struct NovelLineHandler {
    // idx は NovelSourceStream が保持する src の中での byte index であることに注意
    // 行頭からではない
    begin_idx: usize,
    end_idx: usize,
    kind: NovelLineKind,
    last_token_continues_over_line: bool,
}

impl NovelLineHandler {
    pub(super) fn new(begin_idx: usize, end_idx: usize, kind: NovelLineKind) -> Self {
        Self {
            begin_idx,
            end_idx,
            kind,
            last_token_continues_over_line: false,
        }
    }

    #[inline]
    pub(super) fn begin_idx(&self) -> usize {
        self.begin_idx
    }

    #[inline]
    pub(super) fn end_idx(&self) -> usize {
        self.end_idx
    }

    #[inline]
    pub(super) fn proceed_to(&mut self, idx: usize) {
        self.begin_idx = idx;
    }

    #[inline]
    pub(super) fn is_line_end(&self) -> bool {
        self.begin_idx >= self.end_idx
    }

    #[inline]
    pub(super) fn is_end_and_not_continued(&self) -> bool {
        (self.begin_idx >= self.end_idx) && !self.last_token_continues_over_line
    }

    #[inline]
    pub(crate) fn kind(&self) -> &NovelLineKind {
        &self.kind
    }

    #[inline]
    pub(super) fn set_last_token_continues_over_line(&mut self, b: bool) {
        self.last_token_continues_over_line = b;
    }

    #[inline]
    pub(super) fn last_token_continues_over_line(&self) -> bool {
        self.last_token_continues_over_line
    }
}

#[derive(Debug)]
pub(crate) enum NovelLineOption {
    Some(NovelLineHandler),
    None { span: Span },
}

impl<'src> NovelSourceStream<'src> {
    #[inline]
    pub(crate) fn line_str(&self, line_handler: &NovelLineHandler) -> &str {
        &self.src[line_handler.begin_idx..line_handler.end_idx]
    }

    pub(crate) fn line_span(&self, line_handler: &NovelLineHandler) -> Span {
        Span::new(
            self.span.module(),
            self.span.begin() + line_handler.begin_idx,
            self.span.begin() + line_handler.end_idx,
        )
    }

    //
    //  ```biwa
    //  scene foo(g: MyGame) -> MyGame {{
    //      #bar()
    //      #if baz() {
    //          Hello!
    //      }
    //  }}
    //  ```
    //
    //  次の行が存在すれば true を返す
    pub(crate) fn next_line(&mut self) -> NovelLineOption {
        // assert_eq!(self.idx, self.next_line_begin_idx);

        if self.next_line_begin_idx >= self.src.len() {
            NovelLineOption::None {
                span: self.span_from(
                    if self.src.is_empty() {
                        0
                    } else {
                        self.src.len() - 1
                    },
                    0,
                ),
            }
        } else {
            let remain_str = &self.src[self.next_line_begin_idx..];
            let raw_next_line = match remain_str.find('\n') {
                Some(line_end_idx) => &remain_str[..=line_end_idx],
                None => remain_str,
            };

            // `//` で始まるなら行コメントとして行ごと無視する
            if raw_next_line.trim_start().starts_with("//") {
                self.next_line_begin_idx += raw_next_line.len();
                self.next_line()
            } else {
                let start_trimmed_line = raw_next_line.trim_start();
                let both_trimmed_line = start_trimmed_line.trim_end();

                // #, @, } 行の場合の値を初期値に
                let mut start_trimmed_len = raw_next_line.len() - start_trimmed_line.len() + 1;

                let (kind, trimmed_next_line) = match start_trimmed_line.chars().next() {
                    Some('#') => (NovelLineKind::GeneralCommand, &both_trimmed_line[1..]),
                    Some('@') => (NovelLineKind::CharaCommand, &both_trimmed_line[1..]),
                    Some('}') => (NovelLineKind::BlockClose, &both_trimmed_line[1..]),
                    Some(_) | None => {
                        // 現在のインデント位置または空白文字でなくなるまで、
                        // 先頭をtrimする
                        start_trimmed_len = 0;
                        for (i, c) in raw_next_line.char_indices() {
                            if i < self.indent_depth() && c.is_whitespace() {
                                start_trimmed_len = i + 1;
                            } else {
                                break;
                            }
                        }

                        // trim_end せずに最後の改行を残す
                        (NovelLineKind::RawNovel, &raw_next_line[start_trimmed_len..])
                    }
                };

                // 行の種類によらず、 `//` で行コメント開始
                // TODO: RawNovel なら `\//` または `\/\/` はコメントアウトのエスケープ
                let next_line = match trimmed_next_line.split_once("//") {
                    Some((before_comment, _)) => before_comment.trim_end(),
                    None => trimmed_next_line,
                };
                let line_handler = NovelLineHandler::new(
                    self.next_line_begin_idx + start_trimmed_len,
                    self.next_line_begin_idx + start_trimmed_len + next_line.len(),
                    kind,
                );

                self.next_line_begin_idx += raw_next_line.len();
                self.current_line = line_handler.clone();
                NovelLineOption::Some(line_handler)
            }
        }
    }

    // 行内である限り( next_line_begin_idx が同じである限り )、
    // 必ず同じ結果を返す
    pub(super) fn next_line_as_continuing_command(&self) -> Option<(NovelLineHandler, usize)> {
        if self.next_line_begin_idx < self.src.len() {
            let remain_str = &self.src[self.next_line_begin_idx..];
            let raw_next_line = match remain_str.find('\n') {
                Some(line_end_idx) => &remain_str[..=line_end_idx],
                None => remain_str,
            };
            let start_trimmed_line = raw_next_line.trim_start();
            let start_trimmed_len = raw_next_line.len() - start_trimmed_line.len();

            let next_line = match start_trimmed_line.split_once("//") {
                Some((before_comment, _)) => before_comment.trim_end(),
                None => start_trimmed_line.trim_end(),
            };

            Some((
                NovelLineHandler::new(
                    self.next_line_begin_idx + start_trimmed_len,
                    self.next_line_begin_idx + start_trimmed_len + next_line.len(),
                    NovelLineKind::GeneralCommand,
                ),
                //  次の次の行の開始 idx を返す
                self.next_line_begin_idx + raw_next_line.len(),
            ))
        } else {
            None
        }
    }
}
