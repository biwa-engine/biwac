use biwac_ast::{Path, PathSegment, PathSegmentResolution};
use biwac_base::{BiwacError, DiagSpan, ErrorContext, InternedIdent, ModId, PackageId};
use biwac_span::{DefIdKind, GenDefId, Span, TyDefId, ValDefId, VarId};

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
