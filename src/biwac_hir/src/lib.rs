mod error;
pub mod hir;

pub use crate::hir::{
    Hir, ImplValId, PkgId, Progressive, TyExistence,
    symbols::{
        Ident,
        expressions::{
            AssocCallee, BinaryExpr, BlockExpr, Callee, Expr, ExprId, ExprVal, FnCall, IfExpr,
            Literal, MemberAccess, MethodCall, Primary, StructLiteral, UnaryExpr, VarIdKind,
            Variable,
        },
        globals::{
            DecledArg, FnDefContent, FnDefContentBody, FnDefContentSignature, GenTyId,
            ImplValDefContentKind, LocGenTyId, MethodDefContent, NativeCode, NativeFnArgDecl,
            NativeFnDefContent, NativeMethodDefContent, NativeTypeAliasDefContent,
            NovelSceneDefContent, StructDefContent, TyDefContentKind, TypeAliasDefContent,
            ValDefContentKind,
        },
        statements::{
            AssignStmt, BlockStmt, DecledVar, ExprStmt, IfStmt, LocVarId, ReturnStmt, Stmt,
            VarDecl, WhileStmt,
        },
    },
    types::{DefinedTy, FnTy, InferTy, Ty, TyKind, TyVar},
};

pub use error::HirError;

pub type HirResult<T> = Result<T, HirError>;
