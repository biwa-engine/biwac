mod error;
pub mod hir;

pub use crate::hir::{
    DefinedTyImpl, Hir, ImplValId, SpecialTyImpl, TyExistence, TyValImplGenargsContentPair,
    TyValImplList,
    symbols::{
        Ident,
        expressions::{
            AssocCallee, BinaryExpr, BlockExpr, Callee, Expr, ExprId, ExprVal, FnCall, IfExpr,
            Literal, MemberAccess, MethodCall, Primary, StructLiteral, UnaryExpr, VarIdKind,
            Variable,
        },
        globals::{
            AssocValDefKind, DecledArg, FnBody, FnDef, FnSignature, NativeCode, NativeFnArgDecl,
            NativeFnDef, NativeTypeAliasDef, NovelSceneDef, StructDef, TyDefKind, TypeAliasDef,
            ValDefKind,
        },
        statements::{
            AssignStmt, BlockStmt, DecledVar, ExprStmt, IfStmt, NovelWaitStmt, NovelWriteStmt,
            ReturnStmt, Stmt, VarDecl, WhileStmt,
        },
    },
    types::{DefinedTy, FnTy, InferTy, Ty, TyKind, TyVar},
};

pub use error::HirError;

pub type HirResult<T> = Result<T, HirError>;
