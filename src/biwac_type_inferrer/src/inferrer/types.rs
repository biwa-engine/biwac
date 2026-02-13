use std::collections::HashMap;

use biwac_name_resolver::{AbsId, FnTyp, Typ};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyVar(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    Var(TyVar),
    Void, // for function return type
    Int,
    Float,
    Bool,
    Fn(FnTy),
    Struct(AbsId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FnTy {
    pub(crate) args: Vec<Ty>,
    pub(crate) ret: Box<Ty>, // Void means no return
}

#[derive(Debug, Clone)]
pub struct StructTy {
    pub(crate) members: HashMap<String, Ty>,
    pub(crate) vars: Vec<Ty>, // generics
}

// scheme means type scheme
// this realizes generics (parametric polymorphism)
#[derive(Debug, Clone)]
pub struct Scheme {
    pub(crate) vars: Vec<TyVar>,
    pub(crate) ty: Ty,
}

// NOTE: structの存在確認をせずに変換するので注意
impl From<Typ> for Ty {
    fn from(value: Typ) -> Self {
        match value {
            Typ::Int => Ty::Int,
            Typ::Float => Ty::Float,
            Typ::Bool => Ty::Bool,
            Typ::Fn(f) => Ty::Fn(f.into()),
            Typ::Defined(id) => Ty::Struct(id),
        }
    }
}

impl From<FnTyp> for FnTy {
    fn from(value: FnTyp) -> Self {
        Self {
            args: value.args.into_iter().map(|typ| typ.into()).collect(),
            ret: Box::new(Ty::from(*value.ret)),
        }
    }
}
