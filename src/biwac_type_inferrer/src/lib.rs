mod inferrer;

#[cfg(test)]
mod tests;

use biwac_hir::{AssignStmt, Expr, FnTy, HirError, MemberAccess, StructLiteral, Ty, TyId, TyVar};
use biwac_parser::{BinOperator, Ident, UnOperator};

pub use crate::inferrer::context::TyCtx;

#[derive(Debug, Clone)]
pub enum TyError {
    StructLiteralMemberConfliced {
        member1: Box<Ident>,
        member2: Box<Ident>,
    },
    StructLiteralAssignToInexsistentMember {
        tid: Box<TyId>,
        member: Box<Ident>,
    },
    StructLiteralMemberInsufficient {
        sliteral: Box<StructLiteral>,
        insufficient_members: Vec<String>,
    },
    InvalidStructLiteralOnAliasType {
        ty: Ty,
        sliteral: Box<StructLiteral>,
    },
    MethodConfliced {
        ty: Ty,
        method1: Box<Ident>,
        method2: Box<Ident>,
    },
    MethodNotImplemented {
        ty: Ty,
        method: Box<Ident>,
    },
    // SymbolNotCallable {
    //     id: AbsId,
    //     caller: Span,
    // },
    StructNotHasMember {
        tid: TyId,
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

    FnArgLenMismatched(FnTy, FnTy),
    FnGenArgLenMismatched(FnTy, FnTy),
    TypeConfliced(Ty, Ty),
    OccursCheckFailed(TyVar, Ty),
    InsufficientContext,

    HirError(HirError),
}

pub type TyResult<T> = Result<T, TyError>;

impl From<HirError> for TyError {
    fn from(value: HirError) -> Self {
        Self::HirError(value)
    }
}
