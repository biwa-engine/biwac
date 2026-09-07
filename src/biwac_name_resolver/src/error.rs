use biwac_ast::{Path, PathSegment, PathSegmentResolution};
use biwac_base::{BiwacError, DiagSpan, ErrorContext, InternedIdent, ModId, PackageId};
use biwac_span::{DefIdKind, GenDefId, Span, TraitDefId, TyDefId, ValDefId, VarId};

use crate::{AssocNameTreeItem, name_tree::AssocNameTreeItemKind};

#[derive(Debug)]
pub enum ResolveError {
    /// lang item の回収時に検出したエラー。
    LangItem(biwac_lang_item::LangItemError),

    // module 側から既存の symbol との重複を検知した場合
    DuplicatedSymbolAndModuleName {
        name: InternedIdent,
        mod_id: ModId,
        symbol_span: Span,
    },

    // module 内で symbol 同士の重複を検知した場合
    //
    // 重複を伴う変種は span1 を「先に来た方」で揃えている。
    DuplicatedSymbolName {
        name: InternedIdent,
        span1: Span,
        span2: Span,
    },

    // module 内で symbol 同士の重複を検知した場合
    // ただし片方は def_id までしかわからない場合
    DuplicatedSymbolAndDefIdName {
        name: InternedIdent,
        span: Span,
        def_id_kind: DefIdKind,
    },

    DuplicatedStructMember {
        name: InternedIdent,
        span1: Span,
        span2: Span,
    },

    DuplicatedAssociatedItemForGenArgs {
        name: InternedIdent,
        assoc1: AssocNameTreeItem,
        assoc2: AssocNameTreeItem,
    },

    UnexpectedSelfType {
        span: Span,
    },
    UnexpectedSelfVariable {
        span: Span,
    },

    IdentNotFound {
        ident: biwac_ast::Ident,
    },

    PathResolutionFailed {
        path: Box<Path>,
    },

    GenericTypeWithGenArgs {
        path: Box<Path>,
        def_id: GenDefId,
    },

    TypeNotFoundPackageFound {
        path: Box<Path>,
        pkg_id: PackageId,
    },
    TypeNotFoundModuleFound {
        path: Box<Path>,
        mod_id: ModId,
    },
    TypeNotFoundValueFound {
        path: Box<Path>,
        def_id: ValDefId,
    },
    TypeNotFoundVariableFound {
        path: Box<Path>,
        var_id: VarId,
    },
    ValueNotFoundTypeFound {
        path: Box<Path>,
        def_id: TyDefId,
    },
    DuplicatedGenName {
        name: InternedIdent,
        span1: Span,
        span2: Span,
    },
    DuplicatedLocalGenName {
        name: InternedIdent,
        span1: Span,
        span2: Span,
    },
    DuplicatedVariableName {
        id: InternedIdent,
        var1: Span,
        var2: Span,
    },
    CyclingTypeAlias {
        def_id: Box<TyDefId>,
        detected_position: Box<Span>,
    },

    /// パターンの位置に、バリアントではないものが書かれた。
    VariantExpected {
        path: Box<Path>,
    },

    /// バリアントのフィールドに入れ子のパターンが書かれた。
    ///
    /// 今回はネストを入れていない。
    /// 照合を「タグの一致 + 1 段の束縛」に閉じるためである。
    NestedPatternUnsupported {
        span: Span,
    },

    AssocItemNotFoundForGenArgs {
        segment: PathSegment,
    },
    AmbiguousAssocItem {
        segment: PathSegment,
    },

    // ---- trait ----
    /// 型が要る位置に trait が書かれた。
    TypeNotFoundTraitFound {
        path: Box<Path>,
        def_id: TraitDefId,
    },

    /// 値が要る位置に trait が書かれた。
    ValueNotFoundTraitFound {
        path: Box<Path>,
        def_id: TraitDefId,
    },

    /// `impl Foo: Bar` の `Bar` が trait ではなかった。
    TraitExpected {
        path: Box<Path>,
    },

    /// trait の項目が 2 回宣言された。
    DuplicatedTraitItem {
        name: InternedIdent,
        span1: Span,
        span2: Span,
    },

    /// 孤児則違反。
    ///
    /// trait 自身か対象の型のいずれかが自パッケージで定義されていなければならない。
    ForeignTraitImpl {
        span: Span,
    },

    /// 同じ型に同じ trait を、重なる特殊化で 2 回実装した。
    DuplicatedTraitImpl {
        span1: Span,
        span2: Span,
    },

    /// trait impl の項目名が、その型の既存の関連名と衝突した。
    ///
    /// biwa は 1 つの型にぶら下がる名前を一意に保つ。
    /// 曖昧さを解消する構文 (`[T as Gyao]::guee()`) がまだ無いためである。
    TraitImplNameConflict {
        name: InternedIdent,
        span: Span,
    },

    /// trait が宣言した項目が実装されていない。
    MissingTraitItem {
        name: InternedIdent,
        span: Span,
    },

    /// trait が宣言していない項目が実装されている。
    UnknownTraitItem {
        name: InternedIdent,
        span: Span,
    },

    /// 実装のシグニチャが trait の宣言と食い違っている。
    TraitItemSignatureMismatch {
        name: InternedIdent,
        span: Span,
        /// 宣言の側の位置。
        decl_span: Span,
        detail: String,
    },

    /// スコープにある trait のどれにも関連アイテムが見つからなかった。
    TraitAssocNotFound {
        segment: PathSegment,
    },

    /// 複数の trait が同じ名前を提供していて絞れない。
    AmbiguousTraitAssoc {
        segment: PathSegment,
        candidates: Vec<TraitDefId>,
    },

    /// 実装はあるが、その trait が import されていない。
    TraitNotInScope {
        segment: PathSegment,
        candidates: Vec<TraitDefId>,
    },
}

/// [`Span`] を診断のラベル位置に変換する。
fn at(span: &Span) -> DiagSpan {
    DiagSpan::new(span.module(), span.begin(), span.end())
}

/// パスのうち、解決に失敗した最初のセグメント。
///
/// 解決を試みていないセグメントは `resolved_id` が空のままなので、
/// それも失敗として扱う (`unwrap` すると内部エラーで落ちてしまう)。
fn first_unresolved(path: &Path) -> Option<&PathSegment> {
    path.segments
        .iter()
        .find(|s| !matches!(s.resolved_id.get(), Some(PathSegmentResolution::Ok(_))))
}

/// パスの最後のセグメント。パスは空にならないので必ず取れる。
fn last_segment(path: &Path) -> &PathSegment {
    path.segments
        .last()
        .expect("compiler bug: path with no segment")
}

fn ident_str<'a>(ctx: &'a ErrorContext, id: &InternedIdent) -> &'a str {
    ctx.interner.get_str(id).unwrap_or("<unknown>")
}

fn assoc_kind_name(kind: &AssocNameTreeItemKind) -> &'static str {
    match kind {
        AssocNameTreeItemKind::Val(_) => "an associated function",
        AssocNameTreeItemKind::Variant(_) => "an enum variant",
    }
}

fn def_id_kind_name(kind: &DefIdKind) -> &'static str {
    match kind {
        DefIdKind::Package(_) => "a package",
        DefIdKind::Mod(_) => "a module",
        DefIdKind::Ty(_) => "a type",
        DefIdKind::Variant(_) => "an enum variant",
        DefIdKind::Val(_) => "a value",
        DefIdKind::Gen(_) | DefIdKind::LocalGen(_) => "a generic parameter",
        DefIdKind::Var(_) => "a variable",
        DefIdKind::Trait(_) => "a trait",
    }
}

impl BiwacError for ResolveError {
    fn print_error_message(&self, ctx: &biwac_base::ErrorContext) {
        match self {
            Self::LangItem(e) => {
                let diag = ctx.diagnostic(e.message());
                match e.span() {
                    Some(span) => diag.label(at(span), "here").print(),
                    None => diag.print(),
                }
            }

            Self::DuplicatedSymbolAndModuleName {
                name,
                mod_id,
                symbol_span,
            } => {
                let name = ident_str(ctx, name);
                let module = ctx
                    .srcs
                    .mods
                    .get(mod_id)
                    .map(|m| m.modu.file_name())
                    .unwrap_or_else(|| "<unknown>".to_string());

                ctx.diagnostic(format!("`{name}` is defined twice."))
                    .label(at(symbol_span), format!("`{name}` is defined here"))
                    .note(format!("the module `{module}` already takes that name"))
                    .print();
            }

            Self::DuplicatedSymbolName { name, span1, span2 } => {
                let name = ident_str(ctx, name);

                ctx.diagnostic(format!("`{name}` is defined twice."))
                    .label(at(span2), format!("`{name}` is defined again here"))
                    .sub_label(at(span1), "first defined here")
                    .print();
            }

            Self::DuplicatedSymbolAndDefIdName {
                name,
                span,
                def_id_kind,
            } => {
                let name = ident_str(ctx, name);

                ctx.diagnostic(format!("`{name}` is defined twice."))
                    .label(at(span), format!("`{name}` is defined here"))
                    .note(format!(
                        "{} with the same name already exists",
                        def_id_kind_name(def_id_kind)
                    ))
                    .print();
            }

            Self::DuplicatedStructMember { name, span1, span2 } => {
                let name = ident_str(ctx, name);

                ctx.diagnostic(format!("Struct member `{name}` is declared twice."))
                    .label(at(span2), format!("`{name}` is declared again here"))
                    .sub_label(at(span1), "first declared here")
                    .print();
            }

            Self::DuplicatedAssociatedItemForGenArgs {
                name,
                assoc1,
                assoc2,
            } => {
                let name = ident_str(ctx, name);

                // 名前ツリーの側は span を持たないので、位置は示せない。
                // 種別だけでも書いておくと、
                // バリアントと関連関数の衝突がすぐ分かる。
                ctx.diagnostic(format!("`{name}` is defined twice on this type."))
                    .note(format!(
                        "conflicting definitions: {} and {}",
                        assoc_kind_name(&assoc1.kind),
                        assoc_kind_name(&assoc2.kind)
                    ))
                    .print();
            }

            Self::UnexpectedSelfType { span } => {
                ctx.diagnostic("`Self` is not usable here.")
                    .label(
                        at(span),
                        "`Self` refers to the type of the enclosing impl block",
                    )
                    .note("write the type name instead")
                    .print();
            }

            Self::UnexpectedSelfVariable { span } => {
                ctx.diagnostic("`self` is not usable here.")
                    .label(at(span), "`self` is only available in a method")
                    .print();
            }

            Self::IdentNotFound { ident } => {
                let name = ident_str(ctx, &ident.id);

                ctx.diagnostic("Identifier not found.")
                    .label(
                        at(&ident.span),
                        format!("`{name}` is not found in current scope."),
                    )
                    .print();
            }

            Self::PathResolutionFailed { path } => {
                let diag = ctx.diagnostic("Path resolution failed.");

                match first_unresolved(path) {
                    Some(segment) => {
                        let name = ident_str(ctx, &segment.ident.id);
                        diag.label(at(&segment.ident.span), format!("`{name}` is not found."))
                            .print();
                    }
                    // 全部解決できているのにこのエラーが立つのは、
                    // 解決した種別が要求と食い違っている場合である。
                    None => {
                        let segment = last_segment(path);
                        let name = ident_str(ctx, &segment.ident.id);
                        diag.label(
                            at(&segment.ident.span),
                            format!("`{name}` does not resolve to what is needed here."),
                        )
                        .print();
                    }
                }
            }

            Self::GenericTypeWithGenArgs { path, .. } => {
                let segment = last_segment(path);
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic("Generic parameter cannot take generic arguments.")
                    .label(
                        at(&segment.ident.span),
                        format!("`{name}` is a generic parameter, not a generic type"),
                    )
                    .print();
            }

            Self::TypeNotFoundPackageFound { path, .. } => {
                print_expected_ty_but(ctx, path, "a package");
            }
            Self::TypeNotFoundModuleFound { path, .. } => {
                print_expected_ty_but(ctx, path, "a module");
            }
            Self::TypeNotFoundValueFound { path, .. } => {
                print_expected_ty_but(ctx, path, "a value");
            }
            Self::TypeNotFoundVariableFound { path, .. } => {
                print_expected_ty_but(ctx, path, "a variable");
            }

            Self::ValueNotFoundTypeFound { path, .. } => {
                let segment = last_segment(path);
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic("Value expected, but a type was found.")
                    .label(at(&segment.ident.span), format!("`{name}` is a type"))
                    .print();
            }

            Self::DuplicatedGenName { name, span1, span2 } => {
                let name = ident_str(ctx, name);

                ctx.diagnostic(format!("Generic parameter `{name}` is declared twice."))
                    .label(at(span2), format!("`{name}` is declared again here"))
                    .sub_label(at(span1), "first declared here")
                    .print();
            }

            Self::DuplicatedLocalGenName { name, span1, span2 } => {
                let name = ident_str(ctx, name);

                ctx.diagnostic(format!("Generic parameter `{name}` is declared twice."))
                    .label(at(span2), format!("`{name}` is declared again here"))
                    .sub_label(at(span1), "first declared here")
                    .print();
            }

            Self::DuplicatedVariableName { id, var1, var2 } => {
                let name = ident_str(ctx, id);

                ctx.diagnostic(format!("Variable `{name}` is declared twice."))
                    .label(at(var2), format!("`{name}` is declared again here"))
                    .sub_label(at(var1), "first declared here")
                    .print();
            }

            Self::CyclingTypeAlias {
                detected_position, ..
            } => {
                ctx.diagnostic("Type alias refers to itself.")
                    .label(
                        at(detected_position),
                        "expanding this alias never terminates",
                    )
                    .print();
            }

            Self::VariantExpected { path } => {
                let segment = last_segment(path);
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic("Enum variant expected in this pattern.")
                    .label(
                        at(&segment.ident.span),
                        format!("`{name}` is not an enum variant"),
                    )
                    .print();
            }

            Self::NestedPatternUnsupported { span } => {
                ctx.diagnostic("Nested patterns are not supported yet.")
                    .label(at(span), "only a binding or `_` can appear here")
                    .note("bind the value first, then match on it again")
                    .print();
            }

            Self::AssocItemNotFoundForGenArgs { segment } => {
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic("Associated item not found.")
                    .label(
                        at(&segment.ident.span),
                        format!("no impl provides `{name}` for these generic arguments"),
                    )
                    .print();
            }

            Self::AmbiguousAssocItem { segment } => {
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic("Associated item is ambiguous.")
                    .label(
                        at(&segment.ident.span),
                        format!("more than one impl provides `{name}` here"),
                    )
                    .print();
            }

            // ---- trait ----
            Self::TypeNotFoundTraitFound { path, .. } => {
                let segment = last_segment(path);
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic(format!("`{name}` is a trait, not a type."))
                    .label(at(&segment.ident.span), "a type is expected here")
                    .note("a trait describes how a type behaves; it cannot be used as one")
                    .print();
            }

            Self::ValueNotFoundTraitFound { path, .. } => {
                let segment = last_segment(path);
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic(format!("`{name}` is a trait, not a value."))
                    .label(at(&segment.ident.span), "a value is expected here")
                    .print();
            }

            Self::TraitExpected { path } => {
                let segment = last_segment(path);
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic(format!("`{name}` is not a trait."))
                    .label(
                        at(&segment.ident.span),
                        "only a trait can be written after `:` here",
                    )
                    .print();
            }

            Self::DuplicatedTraitItem { name, span1, span2 } => {
                let name = ident_str(ctx, name);

                ctx.diagnostic(format!("`{name}` is declared twice in this trait."))
                    .label(at(span2), format!("`{name}` is declared again here"))
                    .sub_label(at(span1), "first declared here")
                    .print();
            }

            Self::ForeignTraitImpl { span } => {
                ctx.diagnostic("Neither the trait nor the type is defined in this package.")
                    .label(at(span), "this impl is not allowed")
                    .note(
                        "a trait can be implemented only where the trait itself \
                         or the type it is implemented for is defined",
                    )
                    .print();
            }

            Self::DuplicatedTraitImpl { span1, span2 } => {
                ctx.diagnostic("This trait is implemented twice for the same type.")
                    .label(at(span2), "implemented again here")
                    .sub_label(at(span1), "first implemented here")
                    .print();
            }

            Self::TraitImplNameConflict { name, span } => {
                let name = ident_str(ctx, name);

                ctx.diagnostic(format!("`{name}` is already defined on this type."))
                    .label(at(span), format!("`{name}` is defined again here"))
                    .note(
                        "every associated name on a type must be unique, \
                         whether it comes from a direct impl, a trait impl, or an enum variant",
                    )
                    .print();
            }

            Self::MissingTraitItem { name, span } => {
                let name = ident_str(ctx, name);

                ctx.diagnostic(format!("`{name}` is not implemented."))
                    .label(at(span), format!("the trait declares `{name}`"))
                    .print();
            }

            Self::UnknownTraitItem { name, span } => {
                let name = ident_str(ctx, name);

                ctx.diagnostic(format!("The trait does not declare `{name}`."))
                    .label(at(span), format!("`{name}` is implemented here"))
                    .print();
            }

            Self::TraitItemSignatureMismatch {
                name,
                span,
                decl_span,
                detail,
            } => {
                let name = ident_str(ctx, name);

                let diag = ctx
                    .diagnostic(format!(
                        "The signature of `{name}` does not match the trait."
                    ))
                    .label(at(span), detail.clone());

                // 外部パッケージの trait は宣言の位置を持たない
                // (`.biwameta` は自パッケージのファイルしか知らない)。
                if decl_span.is_dummy() {
                    diag.print();
                } else {
                    diag.sub_label(at(decl_span), "declared here").print();
                }
            }

            Self::TraitAssocNotFound { segment } => {
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic(format!("`{name}` is not found."))
                    .label(at(&segment.ident.span), "no impl provides this name")
                    .print();
            }

            Self::AmbiguousTraitAssoc { segment, .. } => {
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic(format!("`{name}` is ambiguous."))
                    .label(
                        at(&segment.ident.span),
                        format!("more than one trait in scope provides `{name}`"),
                    )
                    .print();
            }

            Self::TraitNotInScope { segment, .. } => {
                let name = ident_str(ctx, &segment.ident.id);

                ctx.diagnostic(format!(
                    "`{name}` is provided by a trait that is not in scope."
                ))
                .label(at(&segment.ident.span), format!("`{name}` is used here"))
                .note("import the trait that implements it to make this name visible")
                .print();
            }
        }
    }
}

fn print_expected_ty_but(ctx: &ErrorContext, path: &Path, found: &str) {
    let segment = last_segment(path);
    let name = ident_str(ctx, &segment.ident.id);

    ctx.diagnostic("Type expected, but something else was found.")
        .label(at(&segment.ident.span), format!("`{name}` is {found}"))
        .print();
}
