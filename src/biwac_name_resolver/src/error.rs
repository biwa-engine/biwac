use ariadne::{Color, Label, Report, ReportKind, Source};

use biwac_ast::{Path, PathSegment, PathSegmentResolution};
use biwac_base::{BiwacError, InternedIdent, ModId, PackageId};
use biwac_span::{DefIdKind, GenDefId, LocalGenDefId, Span, TyDefId, ValDefId, VarId};

use crate::AssocNameTreeItem;

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
        def_id1: GenDefId,
        def_id2: GenDefId,
    },
    DuplicatedLocalGenName {
        name: InternedIdent,
        def_id1: LocalGenDefId,
        def_id2: LocalGenDefId,
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

    AssocItemNotFoundForGenArgs {
        segment: PathSegment,
    },
    AmbiguousAssocItem {
        segment: PathSegment,
    },
}

impl BiwacError for ResolveError {
    fn print_error_message(&self, ctx: &biwac_base::ErrorContext) {
        match self {
            // TODO: 他の変種と同様、ariadne によるソース抜粋付きの表示は未実装。
            Self::LangItem(e) => {
                eprintln!("Error: {}", e.message());
            }

            Self::DuplicatedSymbolAndModuleName {
                name,
                mod_id,
                symbol_span,
            } => {
                todo!()
            }

            // module 内で symbol 同士の重複を検知した場合
            Self::DuplicatedSymbolName { name, span1, span2 } => {
                todo!()
            }

            // module 内で symbol 同士の重複を検知した場合
            // ただし片方は def_id までしかわからない場合
            Self::DuplicatedSymbolAndDefIdName {
                name,
                span,
                def_id_kind,
            } => {
                todo!()
            }

            Self::DuplicatedStructMember { name, span1, span2 } => {
                todo!()
            }

            Self::DuplicatedAssociatedItemForGenArgs { assoc1, assoc2 } => {
                todo!()
            }

            Self::UnexpectedSelfType { span } => {
                todo!()
            }
            Self::UnexpectedSelfVariable { span } => {
                todo!()
            }

            Self::IdentNotFound { ident } => {
                let modsrc = ctx.srcs.mods.get(&ident.span.module()).unwrap();

                let file_name = modsrc.modu.file_name();
                let begin = ident.span.begin();
                let end = ident.span.end();
                let ident_str = ctx.interner.get_str(&ident.id).unwrap();

                Report::build(ReportKind::Error, (file_name.as_str(), begin..end))
                    .with_message("Identifier not found.")
                    .with_label(
                        Label::new((file_name.as_str(), begin..end))
                            .with_message(format!("`{ident_str}` is not found in current scope.",))
                            .with_color(Color::Red),
                    )
                    .finish()
                    .print((file_name.as_str(), Source::from(&modsrc.src)))
                    .unwrap();
            }

            Self::PathResolutionFailed { path } => {
                for segment in &path.segments {
                    match segment.resolved_id.get().unwrap() {
                        PathSegmentResolution::Ok(_) => {
                            continue;
                        }
                        PathSegmentResolution::Err => {
                            let modsrc = ctx.srcs.mods.get(&segment.ident.span.module()).unwrap();

                            let file_name = modsrc.modu.file_name();
                            let begin = segment.ident.span.begin();
                            let end = segment.ident.span.end();
                            let ident_str = ctx.interner.get_str(&segment.ident.id).unwrap();

                            Report::build(ReportKind::Error, (file_name.as_str(), begin..end))
                                .with_message("Path resolution failed.")
                                .with_label(
                                    Label::new((file_name.as_str(), begin..end))
                                        .with_message(format!("`{ident_str}` is not found.",))
                                        .with_color(Color::Red),
                                )
                                .finish()
                                .print((file_name.as_str(), Source::from(&modsrc.src)))
                                .unwrap();

                            break;
                        }
                    }
                }

                panic!("compiler bug: path is not tried to resolve")
            }

            Self::GenericTypeWithGenArgs { path, def_id } => {
                todo!()
            }

            Self::TypeNotFoundPackageFound { path, pkg_id } => {
                todo!()
            }
            Self::TypeNotFoundModuleFound { path, mod_id } => {
                todo!()
            }
            Self::TypeNotFoundValueFound { path, def_id } => {
                todo!()
            }
            Self::TypeNotFoundVariableFound { path, var_id } => {
                todo!()
            }
            Self::ValueNotFoundTypeFound { path, def_id } => {
                todo!()
            }
            Self::DuplicatedGenName {
                name,
                def_id1,
                def_id2,
            } => {
                todo!()
            }
            Self::DuplicatedLocalGenName {
                name,
                def_id1,
                def_id2,
            } => {
                todo!()
            }
            Self::DuplicatedVariableName { id, var1, var2 } => {
                todo!()
            }
            Self::CyclingTypeAlias {
                def_id,
                detected_position,
            } => {
                todo!()
            }

            Self::AssocItemNotFoundForGenArgs { segment } => {
                todo!()
            }
            Self::AmbiguousAssocItem { segment } => {
                todo!()
            }
        }
    }
}
