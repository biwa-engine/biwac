use biwac_base::IdentInterner;
use biwac_span::Span;

use crate::token::NCodeToken;

mod error;
mod symbols;
mod token;
mod types;

pub use error::NovelParseError;

const BIWAC_NOVEL_INDENT_STEP_DEPTH: usize = 4;

#[derive(Debug)]
pub struct NovelSourceStream<'src> {
    span: Span,
    // 現在のネストの深さ
    // ネストの深さから理想的なフォーマットでのインデントが決定される
    // 理想的なフォーマットでのインデント位置は、
    // ネストするたびに空白文字 ' ' 4?文字分下がることになっている
    // この位置からのさらなるインデントは、
    // 生ノベルテキストの場合はノベルテキスト自体だとして、表示に反映される
    nest_depth: usize,
    peeked: Option<Option<NCodeToken>>,

    src: &'src str,

    interner: &'src mut IdentInterner,

    idx: usize,                 // DSL部分の文字列スライス src のインデックス
    line_begin_idx: usize,      // 同じく
    next_line_begin_idx: usize, // 同じく
    line_comment_begin: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NovelLineKind {
    RawNovel,       // 生ノベルテキスト
    GeneralCommand, // # 行
    CharaCommand,   // @ 行
    BlockClose,     // } 行
}

impl<'src> NovelSourceStream<'src> {
    pub fn new(src: &'src str, span: Span, interner: &'src mut IdentInterner) -> Self {
        Self {
            span,
            nest_depth: 1, // scene の中であるため1階層分ネスト
            peeked: None,

            src,

            interner,

            idx: 0,
            line_begin_idx: 0,
            next_line_begin_idx: 0, // == line_begin_idx ならまだ検索してない
            line_comment_begin: false,
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
        // assert_eq!(self.idx, self.next_line_begin_idx);

        if self.next_line_begin_idx >= self.src.len() {
            None
        } else {
            self.line_comment_begin = false;
            self.line_begin_idx = self.next_line_begin_idx;
            self.next_line_begin_idx = self.src.len();
            for (i, c) in self.src[self.line_begin_idx..].char_indices() {
                if c == '\n' {
                    self.next_line_begin_idx = self.line_begin_idx + i + 1;
                    break;
                }
            }
            self.idx = self.line_begin_idx;

            let next_line = &self.src[self.line_begin_idx..self.next_line_begin_idx];

            // 現在のインデント位置または空白文字でなくなるまで、
            // 先頭をtrimする
            for (i, c) in next_line.char_indices() {
                if i <= self.indent_depth() && c.is_whitespace() {
                    self.idx += i;
                } else {
                    break;
                }
            }

            // `//` で始まるなら行コメントとして行ごと無視する
            if next_line.trim_start().starts_with("//") {
                self.next_line()
            } else {
                self.line_kind()
            }
        }
    }

    // 行の種類を返す
    // 空白行は、改行のみのノベルテキストとみなす
    fn line_kind(&mut self) -> Option<NovelLineKind> {
        // 空白文字でない位置まで一時的に下げる
        let mut tmp_idx_in_line = 0;
        for (i, c) in self.src[self.line_begin_idx..self.next_line_begin_idx].char_indices() {
            if !c.is_whitespace() || c == '\n' {
                tmp_idx_in_line = i;
                break;
            }
        }

        // NOTE:
        // コマンド行であれば
        // カーソル位置をその次にずらす
        //
        // 各 idx はバイトインデックスなので、文字数を取る chars().nth() には渡せない
        // (マルチバイト文字を含む行で位置がずれる)
        match self.src[self.line_begin_idx + tmp_idx_in_line..]
            .chars()
            .next()
        {
            Some('#') => {
                self.idx = self.line_begin_idx + tmp_idx_in_line + 1;
                Some(NovelLineKind::GeneralCommand)
            }
            Some('@') => {
                self.idx = self.line_begin_idx + tmp_idx_in_line + 1;
                Some(NovelLineKind::CharaCommand)
            }
            Some('}') => {
                self.idx = self.line_begin_idx + tmp_idx_in_line + 1;
                Some(NovelLineKind::BlockClose)
            }
            Some(_) => Some(NovelLineKind::RawNovel),
            None => Some(NovelLineKind::RawNovel),
        }
    }

    fn current_span(&self, token_len: usize) -> Span {
        Span::new(
            self.span.module(),
            self.span.begin() + self.idx,
            self.span.begin() + self.idx + token_len,
        )
    }

    fn indent_depth(&self) -> usize {
        self.nest_depth * BIWAC_NOVEL_INDENT_STEP_DEPTH
    }

    fn indent_enter(&mut self) {
        self.nest_depth += 1;
    }

    fn indent_return(&mut self) {
        self.nest_depth = self.nest_depth.saturating_sub(1);
    }
}
