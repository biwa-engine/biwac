use biwac_base::{ModPath, Pos, Span};

use crate::{FnDefContentSignature, Ty, TyId, TyKind, ValId};

#[derive(Debug, Clone)]
pub struct LangItem {
    pub kind: LangItemKind,
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
        LangItemKind::Ty { tid, .. } => Span::new(
            ModPath::Mod(tid.quals.clone()),
            Pos::new(0, 0),
            Pos::new(0, 0),
        ),
        LangItemKind::Val { vid, .. } => Span::new(
            ModPath::Mod(vid.quals.clone()),
            Pos::new(0, 0),
            Pos::new(0, 0),
        ),
    }
}

macro_rules! lang_item_ty {
    ( $( $qual:literal ),* ; $id:literal ; $genarg_len:literal ) => {
        LangItem::new(
            LangItemKind::Ty {
                tid: TyId {
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
    ( $( $qual:literal ),* ; $id:literal ; [ $( $genarg:expr ),* ] ( $( $arg:expr ),* ) -> $rty:expr ) => {
        {
            let vid = ValId::new(
                vec![
                    $(
                        $qual.to_string()
                    ),*
                ],
                $id.to_string(),
            );
            let span = Span::new(
                ModPath::Mod(vid.quals.clone()),
                Pos::new(0, 0),
                Pos::new(0, 0),
            );

            LangItem::new(
                LangItemKind::Val {
                    vid,
                    val: LangItemVal::Fn{
                        signature: Box::new(FnDefContentSignature {
                            args: vec![
                                $(
                                    $arg
                                ),*
                            ],
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
        lang_item_ty!("std", "game"; "Game"; 2),
        lang_item_ty!("std", "game"; "Character"; 1),
        lang_item_fn!("std", "game", "base_engine"; "write"; 
            [] () -> TyKind::Void),
        lang_item_fn!("std", "game", "base_engine"; "wait"; 
            [] () -> TyKind::Void),
    ]
}
