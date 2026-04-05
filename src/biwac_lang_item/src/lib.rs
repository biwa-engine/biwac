use biwac_hir::{TyId, ValId};

#[derive(Debug, Clone)]
pub struct LangItem {
    pub kind: LangItemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LangItemKind {
    Ty { tid: TyId, genarg_len: usize },
    Val { vid: ValId },
}

impl LangItem {
    fn new(kind: LangItemKind) -> Self {
        Self { kind }
    }
}

macro_rules! ty_id {
    ( $( $qual:literal ),* ; $id:literal ) => {
        TyId::new(
            vec![
                $(
                    $qual.to_string()
                ),*
            ],
            $id.to_string()
        )
    };
}

macro_rules! val_id {
    ( $( $qual:literal ),* ; $id:literal ) => {
        ValId::new(
            vec![
                $(
                    $qual.to_string()
                ),*
            ],
            $id.to_string()
        )
    };
}

pub fn default_lang_items() -> Vec<LangItem> {
    vec![
        LangItem::new(LangItemKind::Ty {
            tid: ty_id!("std", "game"; "Game"),
            genarg_len: 2,
        }),
        LangItem::new(LangItemKind::Ty {
            tid: ty_id!("std", "game"; "Character"),
            genarg_len: 1,
        }),
    ]
}
