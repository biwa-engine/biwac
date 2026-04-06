use biwac_hir::{TyId, ValId};

#[derive(Debug, Clone)]
pub struct LangItem {
    pub kind: LangItemKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LangItemKind {
    Ty { tid: TyId, genarg_len: usize },
    Val { vid: ValId },
}

impl LangItem {
    fn new(kind: LangItemKind, id: String) -> Self {
        Self { kind, id }
    }
}

macro_rules! lang_item_ty {
    ( $( $qual:literal ),* ; $id:literal ; $genarg_len:literal ) => {
        LangItem::new(
            LangItemKind::Ty {
                tid: TyId::new(
                    vec![
                        $(
                            $qual.to_string()
                        ),*
                    ],
                    $id.to_string(),
                ),
                genarg_len: $genarg_len,
            },
            $id.to_string(),
        )
    };
}

pub fn default_lang_items() -> Vec<LangItem> {
    vec![
        lang_item_ty!("std", "game"; "Game"; 2),
        lang_item_ty!("std", "game"; "Character"; 1),
    ]
}
