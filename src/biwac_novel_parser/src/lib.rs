use biwac_base::{Pos, Span};

use crate::token::{CharKind, NCodeTkKindName, NCodeToken};

mod symbols;
mod token;

pub use crate::symbols::{
    NovelScene,
    statements::NStmt,
};

pub enum NovelParseError<'src> {
    InvalidToken {
        expecteds: Vec<NCodeTkKindName>,
        found: Box<NCodeToken<'src>>,
    },
    InvalidChar {
        expecteds: Vec<CharKind>,
        found: CharKind,
        span: Span,
    },
}

#[derive(Debug)]
pub struct NovelSourceStream<'src> {
    span: Span,
    lines: std::str::Lines<'src>,
    // 理想的なフォーマットでのインデント位置
    // ネストするたびに空白文字 ' ' 4?文字分下がることになっている
    // この位置からのさらなるインデントは、
    // 生ノベルテキストの場合はノベルテキスト自体だとして、表示に反映される
    indent_depth: usize,
    cursor: SourceStreamCursor,
    current_line: &'src str,
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
        self.lines.next().map(|next_line| {
            self.current_line = next_line;
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

            self.line_kind()
        })
    }

    // 行の種類を返す
    // 空白行は、改行のみのノベルテキストとみなす
    fn line_kind(&mut self) -> NovelLineKind {
        let line = self.current_line;
        
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
                NovelLineKind::GeneralCommand
            }
            Some('@') => {
                self.cursor.idx = tmp_idx + 1;
                NovelLineKind::CharaCommand
            }
            Some('}') => {
                self.cursor.idx = tmp_idx + 1;
                NovelLineKind::BlockClose
            }
            Some(_) => NovelLineKind::RawNovel,
            None => NovelLineKind::RawNovel,
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
