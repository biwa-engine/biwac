pub mod expressions;
pub mod globals;
pub mod novel;
pub mod statements;

use std::cell::OnceCell;

use biwac_base::{InternedIdent, ModPath};
use biwac_span::{DefIdKind, Span, TyDefId};

use crate::Globals;

#[derive(Debug)]
pub struct ModAst {
    pub modpath: ModPath,
    pub globals: Vec<Globals>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub abs_header: Option<AbsolutePathHeader>,
    pub segments: Vec<PathSegment>,

    /// zst ensures that [`Path`] is created in this module.
    zst: private::PrivateZeroSizeType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbsolutePathHeader {
    Package(Span),
    SelfTyp(SelfTypHeader),
    // Int, Float, Bool and other primitive types...
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfTypHeader {
    pub span: Span,
    pub resolved_id: OnceCell<TyDefId>,
    zst: private::PrivateZeroSizeType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegmentResolution {
    Ok(DefIdKind),
    Err,
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub ident: Ident,

    // pub genargs: Option<GenArgs>,

    //
    pub resolved_id: OnceCell<PathSegmentResolution>,

    /// zst ensures that [`PathSegment`] is created in this module.
    _zst: private::PrivateZeroSizeType,
}

impl Path {
    pub fn new(abs_header: Option<AbsolutePathHeader>, segments: Vec<PathSegment>) -> Self {
        // if segments is empty, it is compiler bug.
        // Not only <identifier> but also <qualified-identifier> must has at least one valid segment
        // in its segments.
        // <qualified-identifier> ::= ("package" "::") (<identifier> "::")* <identifier>
        //     | "Self" ("::" <identifier>)?
        assert!(!segments.is_empty() || abs_header.is_some());

        Self {
            abs_header,
            segments,
            zst: private::PrivateZeroSizeType,
        }
    }

    pub fn span(&self) -> Span {
        match &self.abs_header {
            Some(abs_header) => Span::merge(
                &abs_header.span(),
                &self.segments.last().unwrap().span().clone(),
            ),
            None => {
                if self.segments.len() == 1 {
                    self.segments.first().unwrap().span().clone()
                } else {
                    Span::merge(
                        &self.segments.first().unwrap().span().clone(),
                        &self.segments.last().unwrap().span().clone(),
                    )
                }
            }
        }
    }
}

impl PathSegment {
    pub fn span(&self) -> Span {
        self.ident.span.clone()
    }
}

impl AbsolutePathHeader {
    pub fn span(&self) -> Span {
        match self {
            Self::Package(span) => span.clone(),
            Self::SelfTyp(self_typ) => self_typ.span.clone(),
        }
    }
}

impl From<Ident> for PathSegment {
    fn from(value: Ident) -> Self {
        Self {
            ident: value,
            resolved_id: OnceCell::new(),
            _zst: private::PrivateZeroSizeType,
        }
    }
}

impl PartialEq for PathSegment {
    fn eq(&self, other: &Self) -> bool {
        self.ident == other.ident
    }
}

impl Eq for PathSegment {}

mod private {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(super) struct PrivateZeroSizeType;
}

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct QualifiedId {
//     pub is_from_root: bool,
//     pub quals: Vec<String>,
//     pub id: String,
//     pub span: Span,
// }
//
// impl QualifiedId {
//     pub fn new_type_impl(typ: &TypRepr, id: String, span: Span) -> Self {
//         let (quals, is_from_root) = match &typ.val {
//             TypReprVal::Primitive(p) => match p {
//                 PrimTyp::Int => (vec!["Int".to_string()], false),
//                 PrimTyp::Uint => (vec!["Uint".to_string()], false),
//                 PrimTyp::Float => (vec!["Float".to_string()], false),
//                 PrimTyp::Bool => (vec!["Bool".to_string()], false),
//             },
//             TypReprVal::Defined(deftyp) => {
//                 let mut quals = deftyp.qualid.quals.clone();
//                 quals.push(deftyp.qualid.id.clone());
//
//                 (quals, deftyp.qualid.is_from_root)
//             }
//         };
//
//         Self {
//             is_from_root,
//             quals,
//             id,
//             span,
//         }
//     }
//
//     pub fn from_type(typ: &TypRepr, span: Span) -> Self {
//         let (quals, id, is_from_root) = match &typ.val {
//             TypReprVal::Primitive(p) => match p {
//                 PrimTyp::Int => (vec![], "Int".to_string(), false),
//                 PrimTyp::Uint => (vec![], "Uint".to_string(), false),
//                 PrimTyp::Float => (vec![], "Float".to_string(), false),
//                 PrimTyp::Bool => (vec![], "Bool".to_string(), false),
//             },
//             TypReprVal::Defined(deftyp) => (
//                 deftyp.qualid.quals.clone(),
//                 deftyp.qualid.id.clone(),
//                 deftyp.qualid.is_from_root,
//             ),
//         };
//
//         Self {
//             is_from_root,
//             quals,
//             id,
//             span,
//         }
//     }
//
//     pub fn only_id(&self) -> Option<&String> {
//         if !self.is_from_root && self.quals.is_empty() {
//             Some(&self.id)
//         } else {
//             None
//         }
//     }
// }

impl SelfTypHeader {
    pub fn new(span: Span) -> Self {
        Self {
            span,
            resolved_id: OnceCell::new(),
            zst: private::PrivateZeroSizeType,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub id: InternedIdent,
    pub span: Span,
}
