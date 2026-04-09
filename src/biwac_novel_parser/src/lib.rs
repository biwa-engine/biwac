use biwac_base::{Pos, Span};

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
}

#[derive(Debug)]
pub struct NovelSourceStream<'src> {
    span: Span,
    lines: Vec<&'src str>,
    // 理想的なフォーマットでのインデント位置
    // ネストするたびに空白文字 ' ' 4?文字分下がることになっている
    // この位置からのさらなるインデントは、
    // 生ノベルテキストの場合はノベルテキスト自体だとして、表示に反映される
    indent_depth: usize,
    cursor: SourceStreamCursor,
    peeked: Option<Option<NCodeToken>>,
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
            lines: src.lines().collect(),
            indent_depth: 4,
            cursor: SourceStreamCursor { lidx: 0, idx: 0 },
            peeked: None,
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
        // NOTE: 0 行目から取得するため、
        // 先に現在のlidxで取得してから加算
        self.lines
            .get(self.cursor.lidx)
            .map(|next_line| {
                self.cursor.lidx += 1;
                self.cursor.idx = 0;

                // 現在のインデント位置または空白文字でなくなるまで、
                // 先頭をtrimする
                for (i, c) in next_line.char_indices() {
                    if i <= self.indent_depth && c.is_whitespace() {
                        self.cursor.idx = i;
                    } else {
                        break;
                    }
                }
            })
            .and_then(|_| self.line_kind())
    }

    // 行の種類を返す
    // 空白行は、改行のみのノベルテキストとみなす
    fn line_kind(&mut self) -> Option<NovelLineKind> {
        let line = self.lines.get(self.cursor.lidx)?;

        // 空白文字でない位置まで一時的に下げる
        let mut tmp_idx = self.cursor.idx;
        for (i, c) in line.char_indices() {
            if !c.is_whitespace() {
                tmp_idx = i;
                break;
            }
        }

        // NOTE:
        // コマンド行であれば
        // カーソル位置をその次にずらす
        match line.chars().nth(tmp_idx) {
            Some('#') => {
                self.cursor.idx = tmp_idx + 1;
                Some(NovelLineKind::GeneralCommand)
            }
            Some('@') => {
                self.cursor.idx = tmp_idx + 1;
                Some(NovelLineKind::CharaCommand)
            }
            Some('}') => {
                self.cursor.idx = tmp_idx + 1;
                Some(NovelLineKind::BlockClose)
            }
            Some(_) => Some(NovelLineKind::RawNovel),
            None => Some(NovelLineKind::RawNovel),
        }
    }

    fn current_span(&self, token_len: usize) -> Span {
        if self.cursor.lidx == 0 {
            let modu = self.span.module().clone();
            let lidx = self.span.begin().line();
            let idx = self.span.begin().idx() + self.cursor.idx;

            Span::new(modu, Pos::new(lidx, idx), Pos::new(lidx, idx + token_len))
        } else {
            let modu = self.span.module().clone();
            let lidx = self.span.begin().line() + self.cursor.lidx;
            let idx = self.cursor.idx;

            Span::new(modu, Pos::new(lidx, idx), Pos::new(lidx, idx + token_len))
        }
    }
}
