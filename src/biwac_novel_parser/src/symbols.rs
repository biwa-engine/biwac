use biwac_ast::NovelStmt;

use crate::{NovelParseError, NovelSourceStream};

pub(crate) mod expressions;
pub(crate) mod statements;

impl<'src> NovelSourceStream<'src> {
    pub fn parse(&mut self) -> Result<Vec<NovelStmt>, NovelParseError> {
        let mut stmts = Vec::new();
        while let Some(stmt) = self.consume_statement()? {
            stmts.push(stmt);
        }

        Ok(stmts)
    }
}
