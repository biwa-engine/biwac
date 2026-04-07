use biwac_ast::Stmt;

use crate::{NovelParseError, NovelSourceStream};

pub(crate) mod expressions;
pub(crate) mod statements;

// novel scene は通常のASTにおける 文 <statement>
// の列と同様であり、
// これはパース段階で変換できる
pub struct NovelScene {
    pub stmts: Vec<Stmt>,
}

impl<'src> NovelSourceStream<'src> {
    pub fn parse(&mut self) -> Result<NovelScene, NovelParseError<'src>> {
        let mut stmts = Vec::new();
        while let Some(stmt) = self.consume_statement()? {
            stmts.push(stmt);
        }

        Ok(NovelScene { stmts })
    }
}
