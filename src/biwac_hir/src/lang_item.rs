use std::str::FromStr;

use biwac_base::{ModPath, PackageName};
use biwac_span::Span;

use crate::{DefinedTy, FnDefContentSignature, Ty, TyId, TyKind, ValId};

#[derive(Debug, Clone)]
pub struct LangItem {
    pub kind: LangItemKind,

    #[allow(dead_code)]
    pub id: String,

    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum LangItemKind {
    Ty { tid: TyId, genarg_len: usize },
    Val { vid: ValId, val: LangItemVal },
}

#[derive(Debug, Clone)]
pub enum LangItemVal {
    Fn {
        signature: Box<FnDefContentSignature>,
    },
}

impl LangItem {
    fn new(kind: LangItemKind, id: String) -> Self {
        Self {
            span: dummy_span(&kind),
            kind,
            id,
        }
    }
}

fn dummy_span(kind: &LangItemKind) -> Span {
    // TODO: Span に package を追加
    match kind {
        LangItemKind::Ty { tid, .. } => SSpan::External {
            pkg: tid.pkg.name().clone(),
            modu: ModPath::Mod(tid.quals.clone()),
        },
        LangItemKind::Val { vid, .. } => SSpan::External {
            pkg: vid.pkg.name().clone(),
            modu: ModPath::Mod(vid.quals.clone()),
        },
    }
}

macro_rules! lang_item_ty {
    ( $pkg:literal ; $( $qual:literal ),* ; $id:literal ; $genarg_len:literal ) => {
        LangItem::new(
            LangItemKind::Ty {
                tid: TyId {
                    pkg: crate::PkgId::new(biwac_base::PackageName::from_str($pkg).unwrap()),
                    quals: vec![
                        $(
                            $qual.to_string()
                        ),*
                    ],
                    id: $id.to_string(),
                },
                genarg_len: $genarg_len,
            },
            $id.to_string(),
        )
    };
}

macro_rules! lang_item_fn {
    ( $pkg:literal ; $( $qual:literal ),* ; $id:literal ; [ $( $genarg:expr ),* ] ( $( $a:literal : $aty:expr ),* ) -> $rty:expr ) => {
        {
            let vid = ValId::new(
                crate::PkgId::new(biwac_base::PackageName::from_str($pkg).unwrap()),
                vec![
                    $(
                        $qual.to_string()
                    ),*
                ],
                $id.to_string(),
            );
            let span = crate::SSpan::External{
                pkg: biwac_base::PackageName::from_str($pkg).unwrap(),
                modu: ModPath::Mod(vec![
                    $(
                        $qual.to_string()
                    ),*
                ]),
            };

            LangItem::new(
                LangItemKind::Val {
                    vid,
                    val: LangItemVal::Fn{
                        signature: Box::new(FnDefContentSignature {
                            args: [
                                $(
                                    ($a, $aty)
                                ),*
                            ].into_iter()
                            .map(|(id, kind): (&str, _)| (crate::Ident {
                                    id: id.to_string(),
                                    span: biwac_base::SSpan::External{
                                        pkg: biwac_base::PackageName::from_str($pkg).unwrap(),
                                        modu: ModPath::Mod(vec![
                                            $(
                                                $qual.to_string()
                                            ),*
                                        ]),
                                    },
                                },
                                Ty {
                                    kind,
                                    span: span.clone(),
                                }
                            ))
                            .collect(),
                            rty: Ty {
                                kind: $rty,
                                span: span.clone(),
                            },
                            genargs: [
                                $(
                                    $genarg
                                ),*
                            ].into_iter()
                            .enumerate()
                            .map(|(i, g)| (g, crate::LocGenTyId::new(i)))
                            .collect(),
                            span,
                        })
                    }
                },
                $id.to_string(),
            )
        }
    };
}

pub(crate) fn default_lang_items() -> Vec<LangItem> {
    vec![
        lang_item_ty!("std"; "game"; "Game"; 2),
        lang_item_ty!("std"; "game"; "Character"; 1),
        lang_item_fn!("std"; "game", "base_engine"; "write"; 
            [] (
                "msg": TyKind::Defined(DefinedTy {
                    tid: TyId::new(
                        crate::PkgId::new(PackageName::from_str("std").unwrap()),
                        vec!["types".into(), "string".into(), ], "String".into()
                    ),
                    genargs: Vec::new() 
                })
            ) -> TyKind::Void),
        lang_item_fn!("std"; "game", "base_engine"; "wait"; 
            [] () -> TyKind::Void),
    ]
}
