use biwac_base::Span;

use crate::token::{CharKind, NCodeTkKindName, NCodeToken};

mod symbols;
mod token;
mod types;

#[derive(Debug, Clone)]
pub enum NovelParseError {
    InvalidToken {
        expecteds: Vec<NCodeTkKindName>,
        found: Box<NCodeToken>,
    },
    InvalidChar {
        expecteds: Vec<CharKind>,
        found: CharKind,
        span: Span,
    },
    InvalidLineEnd {
        expecteds: Vec<NCodeTkKindName>,
        span: Span,
    },
    LineEndExpected {
        found: Box<NCodeToken>,
    },
    GeneralCommandLineOnlyPrefix {
        span: Span,
    },
    InvalidCloseLine {
        span: Span,
    },
    CloseLineExpected {
        span: Span,
    },
}

#[derive(Debug)]
pub struct NovelSourceStream<'src> {
    span: Span,
    // 理想的なフォーマットでのインデント位置
    // ネストするたびに空白文字 ' ' 4?文字分下がることになっている
    // この位置からのさらなるインデントは、
    // 生ノベルテキストの場合はノベルテキスト自体だとして、表示に反映される
    indent_depth: usize,
    peeked: Option<Option<NCodeToken>>,

    src: &'src str,

    idx: usize,
    line_begin_idx: usize,
    next_line_begin_idx: usize,
}

#[derive(Debug)]
pub(crate) struct SourceStreamCursor {
    // {{ ... }} 内の行インデックス
    // つまり実際のファイル先頭からの位置は
    // NovelSourceStream<'_>.span.begin をoffsetに計算する必要がある
    lidx: usize,
    idx: usize, // UTF-8 &str としての char index
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NovelLineKind {
    RawNovel,       // 生ノベルテキスト
    GeneralCommand, // # 行
    CharaCommand,   // @ 行
    BlockClose,     // } 行
}

impl<'src> NovelSourceStream<'src> {
    pub fn new(src: &'src str, span: Span) -> Self {
        Self {
            span,
            indent_depth: 4,
            peeked: None,

            src,

            idx: 0,
            line_begin_idx: 0,
            next_line_begin_idx: 0, // == line_begin_idx ならまだ検索してない
        }
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
    fn next_line(&mut self) -> Option<NovelLineKind> {
        assert_eq!(self.idx, self.next_line_begin_idx);

        if self.next_line_begin_idx >= self.src.len() {
            None
        } else {
            self.line_begin_idx = self.next_line_begin_idx;
            self.next_line_begin_idx = self.src.len();
            for (i, c) in self.src[self.line_begin_idx..].char_indices() {
                if c == '\n' {
                    self.next_line_begin_idx = self.line_begin_idx + i;
                }
            }

            let next_line = &self.src[self.line_begin_idx..self.next_line_begin_idx];

            // 現在のインデント位置または空白文字でなくなるまで、
            // 先頭をtrimする
            for (i, c) in next_line.char_indices() {
                if i <= self.indent_depth && c.is_whitespace() {
                    self.idx = self.line_begin_idx + i;
                } else {
                    break;
                }
            }

            self.line_kind()
        }
    }

    fn current_line(&self) -> &str {
        &self.src[self.line_begin_idx..self.next_line_begin_idx]
    }

    // 行の種類を返す
    // 空白行は、改行のみのノベルテキストとみなす
    fn line_kind(&mut self) -> Option<NovelLineKind> {
        let line = self.current_line();

        // 空白文字でない位置まで一時的に下げる
        let mut tmp_idx = self.idx;
        for (i, c) in line.char_indices() {
            if !c.is_whitespace() || c == '\n' {
                tmp_idx = i;
                break;
            }
        }

        // NOTE:
        // コマンド行であれば
        // カーソル位置をその次にずらす
        match line.chars().nth(tmp_idx) {
            Some('#') => {
                self.idx = tmp_idx + 1;
                Some(NovelLineKind::GeneralCommand)
            }
            Some('@') => {
                self.idx = tmp_idx + 1;
                Some(NovelLineKind::CharaCommand)
            }
            Some('}') => {
                self.idx = tmp_idx + 1;
                Some(NovelLineKind::BlockClose)
            }
            Some(_) => Some(NovelLineKind::RawNovel),
            None => Some(NovelLineKind::RawNovel),
        }
    }

    fn current_span(&self, token_len: usize) -> Span {
        Span::new(self.span.module(), self.idx, self.idx + token_len)
    }
}
