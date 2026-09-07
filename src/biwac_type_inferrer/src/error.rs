use std::collections::HashMap;

use biwac_ast::{BinOperator, UnOperator, VariantShape};
use biwac_base::{BiwacError, DiagSpan, InternedIdent};
use biwac_hir::{AssignStmt, Expr, FnTy, Ident, MemberAccess, StructLiteral, Ty, TyKind, TyVar};
use biwac_span::{Span, TyDefId};

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

    /// 実装はあるが、その trait が import されていない。
    MethodNotInScope {
        ty: Box<Ty>,
        method: Box<Ident>,
    },

    /// 複数の trait が同じ名前のメソッドを提供していて絞れない。
    AmbiguousMethod {
        ty: Box<Ty>,
        method: Box<Ident>,
    },

    /// バリアントの書き方が宣言と食い違う
    /// (`Rgb(Int)` を `Rgb { .. }` で作るなど)。
    VariantShapeMismatched {
        declared: VariantShape,
        found: VariantShape,
        span: Span,
    },

    /// タプル形式のバリアントに渡した値の個数が合わない。
    VariantFieldCountMismatched {
        expected: usize,
        found: usize,
        span: Span,
    },

    /// 宣言に無いフィールドが書かれた。
    VariantFieldNotFound {
        field: Box<Ident>,
    },

    /// 書かれていないフィールドがある。
    VariantFieldInsufficient {
        missing: Vec<InternedIdent>,
        span: Span,
    },

    /// `match` の対象が enum ではない。
    MatchOnNonEnum {
        ty: Box<Ty>,
        span: Span,
    },

    /// アームのパターンが、対象の enum のバリアントではない。
    VariantOfAnotherEnum {
        ty: Box<Ty>,
        span: Span,
    },

    /// 網羅していないバリアントがある。
    NonExhaustiveMatch {
        missing: Vec<InternedIdent>,
        span: Span,
    },

    /// 前のアームで既に当たるので、このアームには到達しない。
    UnreachableMatchArm {
        span: Span,
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

/// 型の名前引き表。
///
/// [`TyError`] は [`Ty`] を持つが、型の名前は [`TyDefId`] からしか辿れず、
/// その対応は HIR と依存パッケージのメタデータにしか無い。
/// 診断を出す [`biwac_base::ErrorContext`] はどちらも持っていないので、
/// 型推論を抜けるところで必要な名前だけ引いて畳んでおく。
#[derive(Debug, Clone, Default)]
pub struct TyNames {
    pub tys: HashMap<TyDefId, String>,
    /// ジェネリック引数の宣言名。`GenDefId` / `LocalGenDefId` の生の値で引く。
    pub gens: HashMap<u64, String>,
}

impl TyNames {
    /// 型を biwa の表記に近い形の文字列にする。
    pub fn render(&self, kind: &TyKind) -> String {
        match kind {
            TyKind::Int => "Int".to_string(),
            TyKind::Float => "Float".to_string(),
            TyKind::Bool => "Bool".to_string(),
            TyKind::Void => "void".to_string(),
            // 未確定の型変数。利用者から見れば「まだ決まっていない」でしかない。
            TyKind::Infer(_) => "_".to_string(),
            TyKind::Gen(id) => self.gen_name(id.value()),
            TyKind::LocGen(id) => self.gen_name(id.value()),
            TyKind::Defined(defined_ty) => {
                let name = self
                    .tys
                    .get(&defined_ty.def_id)
                    .cloned()
                    .unwrap_or_else(|| "<unknown type>".to_string());

                if defined_ty.genargs.is_empty() {
                    name
                } else {
                    let genargs = defined_ty
                        .genargs
                        .iter()
                        .map(|g| self.render(&g.kind))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{name}[{genargs}]")
                }
            }
            TyKind::Fn(fty) => {
                let args = fty
                    .args
                    .iter()
                    .map(|a| self.render(&a.kind))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("fn({args}) -> {}", self.render(&fty.rty.kind))
            }
        }
    }

    fn gen_name(&self, value: u64) -> String {
        self.gens
            .get(&value)
            .cloned()
            .unwrap_or_else(|| "_".to_string())
    }
}

/// [`TyError`] にソース抜粋つきの表示を与えるための包み。
///
/// 型推論の中では名前を引く手段が無い箇所からもエラーを作るので、
/// 表示に要る名前は境界でまとめて解決して持たせる。
#[derive(Debug, Clone)]
pub struct TyErrorReport {
    pub error: TyError,
    pub names: TyNames,
}

impl TyErrorReport {
    pub fn new(error: TyError, names: TyNames) -> Self {
        Self { error, names }
    }
}

fn at(span: &Span) -> DiagSpan {
    DiagSpan::new(span.module(), span.begin(), span.end())
}

/// 引数の個数が食い違ったときに指す位置。
///
/// [`FnTy`] 自体は span を持たないので、引数か戻り値のものを借りる。
/// 呼び出し側の型は実際のソース位置を持っている。
fn fn_ty_span(fty: &FnTy) -> &Span {
    fty.args
        .first()
        .map(|a| &a.span)
        .unwrap_or(&fty.rty.as_ref().span)
}

impl BiwacError for TyErrorReport {
    fn print_error_message(&self, ctx: &biwac_base::ErrorContext) {
        let names = &self.names;
        let ident_str = |id: &InternedIdent| -> String {
            ctx.interner.get_str(id).unwrap_or("<unknown>").to_string()
        };

        match &self.error {
            TyError::StructLiteralMemberConfliced { member1, member2 } => {
                let name = ident_str(&member1.id);

                ctx.diagnostic(format!("Member `{name}` is assigned twice."))
                    .label(
                        at(&member2.span),
                        format!("`{name}` is assigned again here"),
                    )
                    .sub_label(at(&member1.span), "first assigned here")
                    .print();
            }

            TyError::StructLiteralAssignToInexsistentMember { def_id, member } => {
                let name = ident_str(&member.id);
                let ty = names
                    .tys
                    .get(def_id)
                    .cloned()
                    .unwrap_or_else(|| "the struct".to_string());

                ctx.diagnostic(format!("`{ty}` has no member `{name}`."))
                    .label(
                        at(&member.span),
                        format!("`{name}` is not a member of `{ty}`"),
                    )
                    .print();
            }

            TyError::StructLiteralMemberInsufficient {
                sliteral,
                insufficient_members,
            } => {
                let missing = insufficient_members
                    .iter()
                    .map(|m| format!("`{}`", ident_str(m)))
                    .collect::<Vec<_>>()
                    .join(", ");

                ctx.diagnostic("Struct literal is missing members.")
                    .label(at(&sliteral.span), format!("{missing} not assigned"))
                    .print();
            }

            TyError::InvalidStructLiteralOnAliasType { ty, sliteral } => {
                ctx.diagnostic("Struct literal cannot be used for this type.")
                    .label(
                        at(&sliteral.span),
                        format!("`{}` is not a struct", names.render(&ty.kind)),
                    )
                    .print();
            }

            TyError::StructNotHasMember { def_id, access } => {
                let name = ident_str(&access.member.id);
                let ty = names
                    .tys
                    .get(def_id)
                    .cloned()
                    .unwrap_or_else(|| "the struct".to_string());

                ctx.diagnostic(format!("`{ty}` has no member `{name}`."))
                    .label(at(&access.member.span), format!("no member `{name}`"))
                    .print();
            }

            TyError::ExprNotHasMember { ty, access } => {
                let name = ident_str(&access.member.id);
                let ty = names.render(&ty.kind);

                ctx.diagnostic(format!("`{ty}` has no member `{name}`."))
                    .label(
                        at(&access.member.span),
                        format!("`{ty}` is not a type with members"),
                    )
                    .print();
            }

            TyError::InvalidBinaryOperationForType { ty, op, expr } => {
                let ty = names.render(&ty.kind);

                ctx.diagnostic(format!("`{op}` cannot be applied to `{ty}`."))
                    .label(at(&expr.span()), format!("this is `{ty}`"))
                    .print();
            }

            TyError::InvalidUnaryOperationForType { ty, op, expr } => {
                let ty = names.render(&ty.kind);

                ctx.diagnostic(format!("`{op}` cannot be applied to `{ty}`."))
                    .label(at(&expr.span()), format!("this is `{ty}`"))
                    .print();
            }

            TyError::InvalidAssignOperation { ass } => {
                ctx.diagnostic("This expression cannot be assigned to.")
                    .label(
                        at(&ass.span),
                        "only a variable or a struct member is assignable",
                    )
                    .print();
            }

            TyError::FnArgLenMismatched(callee, caller) => {
                ctx.diagnostic(format!(
                    "This call takes {} argument(s), but {} were given.",
                    callee.args.len(),
                    caller.args.len()
                ))
                .label(at(fn_ty_span(caller)), "in this call")
                .print();
            }

            TyError::FnGenArgLenMismatched(callee, caller) => {
                ctx.diagnostic(format!(
                    "This call takes {} generic argument(s), but {} were given.",
                    callee.genargs.len(),
                    caller.genargs.len()
                ))
                .label(at(fn_ty_span(caller)), "in this call")
                .print();
            }

            TyError::TypeConfliced { t1, t2 } => {
                let n1 = names.render(&t1.kind);
                let n2 = names.render(&t2.kind);

                // `unify` の第 1 引数は要求している側、第 2 引数は実際の型、
                // という向きに呼び出し側を揃えてある。
                let diag = ctx
                    .diagnostic(format!("Expected `{n1}`, but found `{n2}`."))
                    .label(at(&t2.span), format!("this is `{n2}`"));

                // 期待している型がソースに書かれていない場合
                // (`if` の条件に要求する `Bool` など) は、
                // 位置を実際の型から借りている。同じ場所を 2 度指しても仕方がない。
                if t1.span == t2.span {
                    diag.print();
                } else {
                    diag.sub_label(at(&t1.span), format!("`{n1}` is required here"))
                        .print();
                }
            }

            TyError::OccursCheckFailed { ty, .. } => {
                ctx.diagnostic("This type would contain itself.")
                    .label(
                        at(&ty.span),
                        format!(
                            "inferred as `{}`, which refers to itself",
                            names.render(&ty.kind)
                        ),
                    )
                    .print();
            }

            TyError::InsufficientContext => {
                ctx.diagnostic("Not enough context to determine a type.")
                    .print();
            }

            TyError::ReturnTypeRequired { rty } => {
                let ty = names.render(&rty.kind);

                ctx.diagnostic(format!("This function must return `{ty}`."))
                    .label(at(&rty.span), format!("`{ty}` is declared here"))
                    .print();
            }

            TyError::MethodNotFound { ty, method } => {
                let name = ident_str(&method.id);
                let ty = names.render(&ty.kind);

                ctx.diagnostic(format!("`{ty}` has no method `{name}`."))
                    .label(at(&method.span), format!("no method `{name}` on `{ty}`"))
                    .print();
            }

            TyError::MethodNotInScope { ty, method } => {
                let name = ident_str(&method.id);
                let ty = names.render(&ty.kind);

                ctx.diagnostic(format!(
                    "`{name}` is provided by a trait that is not in scope."
                ))
                .label(at(&method.span), format!("`{name}` is used on `{ty}` here"))
                .note("import the trait that implements it to make this method visible")
                .print();
            }

            TyError::AmbiguousMethod { ty, method } => {
                let name = ident_str(&method.id);
                let ty = names.render(&ty.kind);

                ctx.diagnostic(format!("`{name}` is ambiguous on `{ty}`."))
                    .label(
                        at(&method.span),
                        format!("more than one trait in scope provides `{name}`"),
                    )
                    .print();
            }

            TyError::VariantShapeMismatched {
                declared,
                found,
                span,
            } => {
                ctx.diagnostic(format!(
                    "This variant is declared in {declared} form, but written in {found} form."
                ))
                .label(at(span), format!("write it in {declared} form"))
                .print();
            }

            TyError::VariantFieldCountMismatched {
                expected,
                found,
                span,
            } => {
                ctx.diagnostic(format!(
                    "This variant takes {expected} field(s), but {found} were given."
                ))
                .label(at(span), "here")
                .print();
            }

            TyError::VariantFieldNotFound { field } => {
                let name = ident_str(&field.id);

                ctx.diagnostic(format!("This variant has no field `{name}`."))
                    .label(at(&field.span), format!("no field `{name}`"))
                    .print();
            }

            TyError::VariantFieldInsufficient { missing, span } => {
                let missing = missing
                    .iter()
                    .map(|m| format!("`{}`", ident_str(m)))
                    .collect::<Vec<_>>()
                    .join(", ");

                ctx.diagnostic("Variant is missing fields.")
                    .label(at(span), format!("{missing} not given"))
                    .print();
            }

            TyError::MatchOnNonEnum { ty, span } => {
                let ty = names.render(&ty.kind);

                ctx.diagnostic(format!("`match` needs an enum, but this is `{ty}`."))
                    .label(at(span), format!("this is `{ty}`"))
                    .print();
            }

            TyError::VariantOfAnotherEnum { ty, span } => {
                let ty = names.render(&ty.kind);

                ctx.diagnostic(format!("This pattern is not a variant of `{ty}`."))
                    .label(at(span), format!("`{ty}` is being matched here"))
                    .print();
            }

            TyError::NonExhaustiveMatch { missing, span } => {
                let missing = missing
                    .iter()
                    .map(|m| format!("`{}`", ident_str(m)))
                    .collect::<Vec<_>>()
                    .join(", ");

                ctx.diagnostic("This `match` is not exhaustive.")
                    .label(at(span), format!("{missing} not covered"))
                    .note("add the missing arms, or a `_` arm")
                    .print();
            }

            TyError::UnreachableMatchArm { span } => {
                ctx.diagnostic("This arm is never reached.")
                    .label(at(span), "an earlier arm already covers it")
                    .print();
            }

            TyError::MissingLangItem { item } => {
                ctx.diagnostic(format!(
                    "The lang item `{}` is not provided by any package in the dependency graph.",
                    item.key()
                ))
                .note("depend on `std`, or provide it with `[[lang=\"...\"]]`")
                .print();
            }
        }
    }
}

/// エラーが抱えている型を集める。名前を引く対象を知るために使う。
pub(crate) fn error_tys(error: &TyError) -> Vec<&Ty> {
    match error {
        TyError::InvalidStructLiteralOnAliasType { ty, .. }
        | TyError::ExprNotHasMember { ty, .. }
        | TyError::InvalidBinaryOperationForType { ty, .. }
        | TyError::InvalidUnaryOperationForType { ty, .. }
        | TyError::MethodNotFound { ty, .. }
        | TyError::MethodNotInScope { ty, .. }
        | TyError::AmbiguousMethod { ty, .. }
        | TyError::OccursCheckFailed { ty, .. } => vec![ty],

        TyError::TypeConfliced { t1, t2 } => vec![t1, t2],
        TyError::MatchOnNonEnum { ty, .. } | TyError::VariantOfAnotherEnum { ty, .. } => vec![ty],
        TyError::ReturnTypeRequired { rty } => vec![rty],

        TyError::FnArgLenMismatched(f1, f2) | TyError::FnGenArgLenMismatched(f1, f2) => f1
            .args
            .iter()
            .chain(std::iter::once(f1.rty.as_ref()))
            .chain(f2.args.iter())
            .chain(std::iter::once(f2.rty.as_ref()))
            .collect(),

        TyError::VariantShapeMismatched { .. }
        | TyError::VariantFieldCountMismatched { .. }
        | TyError::VariantFieldNotFound { .. }
        | TyError::VariantFieldInsufficient { .. }
        | TyError::NonExhaustiveMatch { .. }
        | TyError::UnreachableMatchArm { .. }
        | TyError::StructLiteralMemberConfliced { .. }
        | TyError::StructLiteralAssignToInexsistentMember { .. }
        | TyError::StructLiteralMemberInsufficient { .. }
        | TyError::StructNotHasMember { .. }
        | TyError::InvalidAssignOperation { .. }
        | TyError::InsufficientContext
        | TyError::MissingLangItem { .. } => Vec::new(),
    }
}

/// エラーが型そのものではなく [`TyDefId`] で抱えている型。
pub(crate) fn error_ty_def_ids(error: &TyError) -> Vec<TyDefId> {
    match error {
        TyError::StructLiteralAssignToInexsistentMember { def_id, .. } => vec![**def_id],
        TyError::StructNotHasMember { def_id, .. } => vec![*def_id],
        _ => Vec::new(),
    }
}
