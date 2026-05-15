pub mod expressions;
pub mod globals;
pub mod statements;

use biwac_ast::ModAst;

use crate::{ParseError, Parser, TokenStream};

impl<'src> Parser<'src> {
    pub fn try_parse(self) -> Result<ModAst, ParseError<'src>> {
        let mut stream =
            TokenStream::new(self.mod_id, self.tokens.iter().peekable(), self.interner);

        let mut globals = vec![];

        loop {
            let g = stream.opt_consume_global_symbols()?;
            match g {
                Some(g) => {
                    globals.push(g);
                }
                None => {
                    break;
                }
            }
        }

        Ok(ModAst {
            modpath: self.modpath,
            globals,
        })
    }
}
