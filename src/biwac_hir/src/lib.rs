pub mod hir;

// バリアントの書き方は AST と HIR で同じものを使う。
// 依存メタデータのように biwac_ast を引かないクレートからも要るので、
// ここから再輸出しておく。
pub use biwac_ast::VariantShape;

pub use crate::hir::{
    DefinedTyImpl, Hir, TyExistence, TyTraitImpl, TyValImplGenargsContentPair, TyValImplList,
    symbols::{
        Ident,
        expressions::{
            BinaryExpr, BlockExpr, Callee, Expr, ExprId, ExprVal, FieldBinding, FnCall, IfExpr,
            Literal, MatchExpr, MatchExprArm, MemberAccess, MethodCall, Pattern, PatternFields,
            Primary, ResolvedVariant, StructLiteral, UnaryExpr, VarIdKind, Variable, VariantCtor,
            VariantCtorFields, VariantPattern,
        },
        globals::{
            AssocValDefKind, DecledArg, EnumDef, FnArgDecl, FnBody, FnDef, FnSignature, NativeCode,
            NativeFnArgDecl, NativeFnDef, NativeTypeAliasDef, NovelSceneDef, StructDef,
            TraitAssocOwner, TraitCond, TraitCondList, TraitDef, TraitItemDef, TyDefKind,
            TypeAliasDef, ValDefKind, VariantDef, VariantOwner,
        },
        statements::{
            AssignStmt, BlockStmt, DecledVar, ExprStmt, IfStmt, MatchStmt, MatchStmtArm,
            NovelWaitStmt, NovelWriteStmt, ReturnStmt, Stmt, VarDecl, WhileStmt,
        },
    },
    types::{DefinedTy, FnTy, InferTy, Ty, TyKind, TyVar},
};
