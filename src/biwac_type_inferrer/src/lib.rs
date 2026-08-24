mod inferrer;

#[cfg(test)]
mod tests;

use biwac_ast::{BinOperator, UnOperator};
use biwac_base::InternedIdent;
use biwac_hir::{AssignStmt, Expr, FnTy, Ident, MemberAccess, StructLiteral, Ty, TyVar};
use biwac_span::TyDefId;

pub use crate::inferrer::context::TyCtx;

#[derive(Debug, Clone)]
pub enum TyError {
    StructLiteralMemberConfliced {
        member1: Box<Ident>,
        member2: Box<Ident>,
    },
    StructLiteralAssignToInexsistentMember {
        def_id: Box<TyDefId>,
        member: Box<Ident>,
    },
    StructLiteralMemberInsufficient {
        sliteral: Box<StructLiteral>,
        insufficient_members: Vec<InternedIdent>,
    },
    InvalidStructLiteralOnAliasType {
        ty: Box<Ty>,
        sliteral: Box<StructLiteral>,
    },
    StructNotHasMember {
        def_id: TyDefId,
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

    MethodNotFound {
        ty: Box<Ty>,
        method: Box<Ident>,
    },

    /// コンパイラが必要とする lang item が定義されていない。
    ///
    /// no_std パッケージのビルドでは回収パスが完全性を検証するため、
    /// ここに到達するのは依存パッケージが lang item を提供していない場合
    /// (例: std に依存していない、または推移的依存の先にしか std がない) である。
    MissingLangItem {
        item: biwac_lang_item::LangItem,
    },
}

pub type TyResult<T> = Result<T, TyError>;
