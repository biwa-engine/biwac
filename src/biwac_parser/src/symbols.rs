pub mod expressions;
pub mod globals;
pub mod statements;

use biwac_ast::ModAst;

use crate::{ParseError, Parser, TokenStream};

impl Parser {
    pub fn try_parse(self) -> Result<ModAst, ParseError> {
        let mut stream = TokenStream::new(self.tokens.iter().peekable());

        let mut globals = vec![];

        loop {
            let gs = stream.opt_consume_global_symbols()?;
            if gs.is_empty() {
                break;
            } else {
                globals.extend(gs);
            }
        }

        Ok(ModAst {
            modpath: self.modpath,
            globals,
        })
    }
}
