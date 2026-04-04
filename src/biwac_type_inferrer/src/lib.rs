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
        ty: Box<Ty>,
        sliteral: Box<StructLiteral>,
    },
    MethodNotImplemented {
        ty: Box<Ty>,
        method: Box<Ident>,
    },
    StructNotHasMember {
        tid: TyId,
        access: Box<MemberAccess>,
    },
    ExprNotHasMember {
        ty: Box<Ty>,
        access: Box<MemberAccess>,
    },
    InvalidBinaryOperationForType {
        ty: Box<Ty>,
        op: BinOperator,
        expr: Box<Expr>,
    },
    InvalidUnaryOperationForType {
        ty: Box<Ty>,
        op: UnOperator,
        expr: Box<Expr>,
    },
    InvalidAssignOperation {
        ass: Box<AssignStmt>,
        // only variable and struct member access left hand side is assignable
    },

    FnArgLenMismatched(FnTy, FnTy),
    FnGenArgLenMismatched(FnTy, FnTy),

    TypeConfliced {
        t1: Box<Ty>,
        t2: Box<Ty>,
    },
    OccursCheckFailed {
        tv: Box<TyVar>,
        ty: Box<Ty>,
    },
    InsufficientContext,
    ReturnTypeRequired {
        rty: Box<Ty>, // 関数が要求する戻り値
    },

    HirError(HirError),
}

pub type TyResult<T> = Result<T, TyError>;

impl From<HirError> for TyError {
    fn from(value: HirError) -> Self {
        Self::HirError(value)
    }
}
