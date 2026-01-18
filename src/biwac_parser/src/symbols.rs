pub mod expressions;
pub mod globals;
pub mod statements;

use biwac_lexer::Token;

use crate::{ParseError, parser::TokenStream, symbols::globals::Globals};

#[derive(Debug)]
pub struct ModAst {
    pub globals: Vec<Globals>, // pub fns: Vec<FnDec>,
}

impl ModAst {
    pub fn try_parse(tokens: Vec<Token>) -> Result<Self, ParseError> {
        let mut stream = TokenStream::new(tokens.iter().peekable());

        let mut globals = vec![];

        while let Some(global) = stream.opt_consume_global_symbol()? {
            globals.push(global);
        }

        Ok(Self { globals })
    }
}

#[derive(Debug, Clone)]
pub struct QualifiedId {
    pub is_from_root: bool,
    pub quals: Vec<String>,
    pub id: String,
}
