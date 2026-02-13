mod inferrer;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use biwac_base::Span;
use biwac_name_resolver::{
    AbsId, AssignStmt, DecledArg, DecledVar, Expr, ExprId, GlobalVarDecl, LocVarId, MemberAccess,
    Stmt, StructLiteral, TypeDefContent,
};
use biwac_parser::{BinOperator, Ident, UnOperator};
pub use inferrer::{
    infer,
    types::{FnTy, Scheme, Ty, TyVar},
};

#[derive(Debug, Clone, PartialEq)]
pub enum TyError {
    SymbolNotFound(AbsId),
    StructMemberConfliced {
        id: AbsId,
        member1: Box<Ident>,
        member2: Box<Ident>,
    },
    StructLiteralMemberConfliced {
        member1: Box<Ident>,
        member2: Box<Ident>,
    },
    StructLiteralAssignToInexsistentMember {
        id: AbsId,
        member: Box<Ident>,
    },
    StructLiteralMemberInsufficient {
        sliteral: Box<StructLiteral>,
        insufficient_members: Vec<String>,
    },
    MethodConfliced {
        ty: Ty,
        method1: Box<Ident>,
        method2: Box<Ident>,
    },
    SymbolNotAType {
        id: AbsId,
    },
    SymbolNotCallable {
        id: AbsId,
        caller: Span,
    },
    SymbolNotAStruct {
        id: AbsId,
        span: Span,
    },
    SymbolNotHasMember {
        id: AbsId,
        access: Box<MemberAccess>,
    },
    StructNotHasMember {
        id: AbsId,
        access: Box<MemberAccess>,
    },
    ExprNotHasMember {
        ty: Ty,
        access: Box<MemberAccess>,
    },
    InvalidBinaryOperationForType {
        ty: Ty,
        op: BinOperator,
        expr: Box<Expr>,
    },
    InvalidUnaryOperationForType {
        ty: Ty,
        op: UnOperator,
        expr: Box<Expr>,
    },
    InvalidAssignOperation {
        ass: Box<AssignStmt>,
        // only variable and struct member access left hand side is assignable
    },

    TypeVariableNotCallable(TyVar),
    FnArgLenMismatched(FnTy, FnTy),
    TypeConfliced(Ty, Ty),
    OccursCheckFailed(TyVar, Ty),
    InsufficientContext,
}

pub type TyResult<T> = Result<T, TyError>;

#[derive(Debug)]
pub struct TypedPkg {
    pub syms: HashMap<AbsId, Sym>,
}

#[derive(Debug)]
pub enum Sym {
    FnDef(FnDefContent),
    VarDecl(GlobalVarDecl),
    TypeDef(TypeDefContent),
}

#[derive(Debug)]
pub struct FnDefContent {
    pub args: Vec<DecledArg>,
    pub stmts: Vec<Stmt>,
    pub expr: Option<Expr>,
    pub rtype: Option<Ty>, // None means void
    pub vars: HashMap<LocVarId, DecledVar>,
    // 推論結果
    pub ty_info: TyInfo,
}
// 関数ローカルな型についての情報
#[derive(Debug)]
pub struct TyInfo {
    pub(crate) vars: HashMap<LocVarId, Ty>,
    pub(crate) exprs: HashMap<ExprId, Ty>,
}

impl TyInfo {
    pub fn unwrap_type_of_expression(&self, expr: &ExprId) -> &Ty {
        self.exprs.get(expr).unwrap()
    }

    pub fn unwrap_type_of_variable(&self, var: &LocVarId) -> &Ty {
        self.vars.get(var).unwrap()
    }
}
