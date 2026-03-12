pub mod hir;

use biwac_base::Span;
use biwac_parser::Ident;

pub use crate::hir::{
    Hir, ImplValId, Progressive, TyExistence,
    symbols::{
        expressions::{
            AssocCallee, BinaryExpr, BlockExpr, Callee, Expr, ExprId, ExprVal, FnCall, IfExpr,
            Literal, MemberAccess, MethodCall, Primary, StructLiteral, UnaryExpr, VarIdKind,
            Variable,
        },
        globals::{
            DecledArg, FnDefContent, FnDefContentBody, FnDefContentSignature, GenTyId,
            ImplValDefContentKind, LocGenTyId, MethodDefContent, NativeFnArgDecl,
            NativeFnDefContent, StructDefContent, TyDefContentKind, TyId, ValDefContentKind, ValId,
        },
        statements::{
            AssignStmt, BlockStmt, DecledVar, ExprStmt, IfStmt, LocVarId, ReturnStmt, Stmt,
            VarDecl, WhileStmt,
        },
    },
    types::{DefinedTy, FnTy, InferTy, Ty, TyVar},
};

pub type HirResult<T> = Result<T, HirError>;

#[derive(Debug, Clone)]
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
    GenericArgLengthMismatched {
        defined_ty: Box<DefinedTy>,
        ty_existence: Box<TyExistence>,
    },
    DuplicatedImplementationForType {
        defined_ty: Box<DefinedTy>,
        ty_existence: Box<TyExistence>,
        val_content1: Box<ImplValDefContentKind>,
        val_content2: Box<ImplValDefContentKind>,
    },
    DuplicatedImplementationForSpecialType {
        ty: Box<Ty>,
        val_content1: Box<ImplValDefContentKind>,
        val_content2: Box<ImplValDefContentKind>,
    },
    ImplementedValueIsNotMethod {
        ty: Box<Ty>,
        method: Box<Ident>,                      // caller のspanを含む
        val_content: Box<ImplValDefContentKind>, // 取得された実装
    },
    ImplementedValueIsNotAssoc {
        ty: Box<Ty>,
        assoc: Box<Ident>,                       // caller のspanを含む
        val_content: Box<ImplValDefContentKind>, // 取得された実装
    },
}
