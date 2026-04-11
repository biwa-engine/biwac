pub mod hir;
mod lang_item;

use biwac_ast::Ident;
use biwac_base::Span;

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
            ImplValDefContentKind, LocGenTyId, MethodDefContent, NativeCode, NativeFnArgDecl,
            NativeFnDefContent, NativeMethodDefContent, NativeTypeAliasDefContent,
            NovelSceneDefContent, StructDefContent, TyDefContentKind, TyId, TypeAliasDefContent,
            ValDefContentKind, ValId,
        },
        statements::{
            AssignStmt, BlockStmt, DecledVar, ExprStmt, IfStmt, LocVarId, ReturnStmt, Stmt,
            VarDecl, WhileStmt,
        },
    },
    types::{DefinedTy, FnTy, InferTy, Ty, TyKind, TyVar},
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
        ty: Box<TyKind>,
        val_content1: Box<ImplValDefContentKind>,
        val_content2: Box<ImplValDefContentKind>,
    },
    ImplementedValueIsNotMethod {
        ty: Box<TyKind>,
        method: Box<Ident>,                      // caller のspanを含む
        val_content: Box<ImplValDefContentKind>, // 取得された実装
    },
    ImplementedValueIsNotAssoc {
        ty: Box<TyKind>,
        assoc: Box<Ident>,                       // caller のspanを含む
        val_content: Box<ImplValDefContentKind>, // 取得された実装
    },
}
