use biwac_span::{InternedIdent, Span};

pub(crate) mod expressions;
pub(crate) mod globals;
pub(crate) mod statements;

#[derive(Debug, Clone)]
pub struct Ident {
    pub id: InternedIdent,
    pub span: Span,
}

impl From<biwac_ast::Ident> for Ident {
    fn from(value: biwac_ast::Ident) -> Self {
        Self {
            id: value.id,
            span: value.span,
        }
    }
}
