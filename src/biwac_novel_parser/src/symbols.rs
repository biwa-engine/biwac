use biwac_ast::NovelStmt;

use crate::{
    NovelLineKind, NovelParseError, NovelSourceStream, symbols::statements::ParsedNovelStmt,
};

pub(crate) mod expressions;
pub(crate) mod statements;

impl<'src> NovelSourceStream<'src> {
    pub fn parse(&mut self) -> Result<Vec<NovelStmt>, NovelParseError> {
        let mut stmts = Vec::new();
        loop {
            let parsed = self.consume_statements()?;

            match parsed {
                ParsedNovelStmt::Stmts { stmts: ss } => {
                    stmts.extend(ss);
                }
                ParsedNovelStmt::ExitBlock { line_handler } => {
                    assert_eq!(line_handler.kind(), &NovelLineKind::BlockClose);

                    return Err(NovelParseError::InvalidCloseLine {
                        span: self.line_span(&line_handler),
                    });
                }
                ParsedNovelStmt::EndOfRange { .. } => {
                    return Ok(stmts);
                }
            }
        }
    }
}
