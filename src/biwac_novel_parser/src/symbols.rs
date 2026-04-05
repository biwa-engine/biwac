use crate::{NStmt, NovelParseError, NovelSourceStream};

pub(crate) mod expressions;
pub(crate) mod statements;

pub struct NovelScene {
    pub stmts: Vec<NStmt>,
}

impl<'src> NovelSourceStream<'src> {
    pub fn parse(&mut self) -> Result<NovelScene, NovelParseError<'src>> {
        let mut stmts = Vec::new();
        while let Some(stmt) = self.parse_statement()? {
            stmts.push(stmt);
        }

        Ok(NovelScene { stmts })
    }
}
