pub mod hir;

use biwac_base::Span;

pub use crate::hir::{
    Hir, Progressive,
    symbols::{
        expressions::{
            BlockExpr, Callee, Expr, ExprId, ExprVal, FnCall, Literal, MemberAccess, Primary,
            StructLiteral, Variable,
        },
        globals::{
            DecledArg, FnDefContent, FnDefContentBody, GenTyId, ImplValDefContentKind, LocGenTyId,
            MethodDefContent, NativeFnArgDecl, NativeFnDefContent, StructDefContent,
            TyDefContentKind, TyId, ValDefContentKind, ValId,
        },
        statements::{
            AssignStmt, BlockStmt, DecledVar, ExprStmt, IfStmt, LocVarId, ReturnStmt, Stmt,
            VarDecl, WhileStmt,
        },
    },
    types::Ty,
};

pub type HirResult<T> = Result<T, HirError>;

pub enum HirError {
    DuplicatedValueName {
        vid: Box<ValId>,
        defined_position1: Box<Span>,
        defined_position2: Box<Span>,
    },
    DuplicatedTypeName {
        tid: Box<TyId>,
        defined_position1: Box<Span>,
        defined_position2: Box<Span>,
    },
}
