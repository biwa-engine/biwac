use biwac_ast::NovelStmt;

use crate::{NovelLineKind, NovelParseError, NovelSourceStream};

pub(crate) mod expressions;
pub(crate) mod statements;

impl<'src> NovelSourceStream<'src> {
    pub fn parse(&mut self) -> Result<Vec<NovelStmt>, NovelParseError> {
        let mut stmts = Vec::new();
        loop {
            let ss = self.consume_statements()?;

            if ss.is_empty() {
                // 正常に最終行までパースできる場合
                //  ```biwa
                //      Hello!
                //  }}            // span.end().line() == span.begin().line() + self.cursor.lidx
                //  ```
                //
                // } が残っていて、パースしようとしても空が返る場合
                //  ```biwa
                //      Hello!
                //      }         // span.begin().line() + self.cursor.lidx
                //  }}            // span.end().line()
                //  ```
                if self.span.end().line() == self.span.begin().line() + self.cursor.lidx {
                    return Ok(stmts);
                } else {
                    assert_eq!(self.line_kind(), Some(NovelLineKind::BlockClose));

                    return Err(NovelParseError::InvalidCloseLine {
                        span: self.current_span(1),
                    });
                }
            } else {
                stmts.extend(ss);
            }
        }
    }
}
