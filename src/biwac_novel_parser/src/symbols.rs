use biwac_ast::Stmt;

use crate::{NovelParseError, NovelSourceStream};

pub(crate) mod expressions;
pub(crate) mod statements;

impl<'src> NovelSourceStream<'src> {
    pub fn parse(&mut self) -> Result<Vec<Stmt>, NovelParseError> {
        let mut stmts = Vec::new();
        while let Some(stmt) = self.consume_statement()? {
            stmts.push(stmt);
        }

        Ok(stmts)
    }
}
