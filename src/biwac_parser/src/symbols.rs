pub mod expressions;
pub mod globals;
pub mod statements;

use biwac_base::Span;
use biwac_lexer::Token;

use crate::{
    ParseError, PrimTyp, TypRepr, TypReprVal, parser::TokenStream, symbols::globals::Globals,
};

#[derive(Debug)]
pub struct ModAst {
    pub globals: Vec<Globals>,
}

impl ModAst {
    pub fn try_parse(tokens: Vec<Token>) -> Result<Self, ParseError> {
        let mut stream = TokenStream::new(tokens.iter().peekable());

        let mut globals = vec![];

        loop {
            let gs = stream.opt_consume_global_symbols()?;
            if gs.is_empty() {
                break;
            } else {
                globals.extend(gs);
            }
        }

        Ok(Self { globals })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedId {
    pub is_from_root: bool,
    pub quals: Vec<String>,
    pub id: String,
    pub span: Span,
}

impl QualifiedId {
    pub fn new_type_impl(typ: &TypRepr, id: String, span: Span) -> Self {
        let (quals, is_from_root) = match &typ.val {
            TypReprVal::Primitive(p) => match p {
                PrimTyp::Int => (vec!["Int".to_string()], false),
                PrimTyp::Uint => (vec!["Uint".to_string()], false),
                PrimTyp::Bool => (vec!["Bool".to_string()], false),
            },
            TypReprVal::Defined(deftyp) => {
                let mut quals = deftyp.qualid.quals.clone();
                quals.push(deftyp.qualid.id.clone());

                (quals, deftyp.qualid.is_from_root)
            }
        };

        Self {
            is_from_root,
            quals,
            id,
            span,
        }
    }

    pub fn from_type(typ: &TypRepr, span: Span) -> Self {
        let (quals, id, is_from_root) = match &typ.val {
            TypReprVal::Primitive(p) => match p {
                PrimTyp::Int => (vec![], "Int".to_string(), false),
                PrimTyp::Uint => (vec![], "Uint".to_string(), false),
                PrimTyp::Bool => (vec![], "Bool".to_string(), false),
            },
            TypReprVal::Defined(deftyp) => (
                deftyp.qualid.quals.clone(),
                deftyp.qualid.id.clone(),
                deftyp.qualid.is_from_root,
            ),
        };

        Self {
            is_from_root,
            quals,
            id,
            span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub id: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprOrStmt<E, S> {
    Expr(E),
    Stmt(S),
}
