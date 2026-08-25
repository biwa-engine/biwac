use std::collections::{HashMap, hash_map::Entry};

use biwac_span::{DefId, Span};

// lang item は「コンパイラが DefId を知っていなければならない定義」の登録簿である。
//
// rustc と同じく、ライブラリ側が
//  ```biwa
//  [[lang="game"]]
//  struct Game[C, S] { ... }
//  ```
// のように名乗り出て、コンパイラが回収・検証する。
// コンパイラ側にパスをハードコードしない。
//
// 名前解決には一切関与しない。
// `String` などが import なしで解決されるのは prelude の仕事であり、
// lang item とは独立した機構である。
//
// 実装がオプショナルな lang item や
// 「依存のどこかにあればよい」lang item は想定しない。
// no_std パッケージ (= std 自身) のビルド時に
// [`LangItemTable::complete`] で全項目が揃っていることを検証する。

/// `LangItem` から `DefId` への写像。
///
/// 依存パッケージ由来の項目を先に登録し、その上に自パッケージの項目を重ねる。
/// 全単射を保つため、同一 lang item の二重定義はエラーとする。
/// これによりユーザパッケージが std の lang item を再定義すると
/// [`LangItemError::DuplicatedDefinition`] になる。
#[derive(Debug, Clone, Default)]
pub struct LangItemTable {
    // FIXME:
    //
    // after std::mem::variant_count::<T>() become stable
    // items: [Option<DefId>; std::mem::variant_count::<LangItem>()],
    //
    // now, instead use HashMap<K, V>
    items: HashMap<LangItem, DefId>,
}

#[derive(Debug)]
pub enum LangItemError {
    /// 同一の lang item が二重に定義された。
    ///
    /// 先に登録された側は依存パッケージ由来である場合があり、
    /// その場合 span を持たないため DefId のみを保持する。
    /// (既存の `ResolveError::DuplicatedSymbolAndDefIdName` と同じ流儀)
    DuplicatedDefinition {
        item: LangItem,
        previous: DefId,
        span: Span,
    },

    /// no_std パッケージのビルドで lang item の実装が欠けている。
    MissingDefinition { item: LangItem },

    /// `[[lang="..."]]` のキーが一覧に無い。
    UnknownKey { key: String, span: Span },

    /// lang item の種別 (型 / 関数) が付与対象と一致しない。
    /// 例: 関数用の `write` を struct に付けた。
    KindMismatch {
        item: LangItem,
        found: LangItemKind,
        span: Span,
    },

    /// ジェネリック引数の個数が要求と一致しない。
    GenericsMismatch {
        item: LangItem,
        found: usize,
        span: Span,
    },
}

impl LangItemError {
    pub fn message(&self) -> String {
        match self {
            Self::DuplicatedDefinition { item, .. } => {
                format!("lang item `{}` is defined more than once", item.key())
            }
            Self::MissingDefinition { item } => format!(
                "this no_std package requires the `{}` lang item to be defined",
                item.key()
            ),
            Self::UnknownKey { key, .. } => {
                let mut known: Vec<&str> = LangItem::ALL.iter().map(|i| i.key()).collect();
                known.sort_unstable();
                format!("unknown lang item `{}` (known: {})", key, known.join(", "))
            }
            Self::KindMismatch { item, found, .. } => format!(
                "lang item `{}` must be {}, but it is applied to {}",
                item.key(),
                item.kind().describe(),
                found.describe()
            ),
            Self::GenericsMismatch { item, found, .. } => match item.required_generics() {
                LangItemGenericRequirement::None => {
                    unreachable!("lang item `{}` has no generics requirement", item.key())
                }
                LangItemGenericRequirement::Exact(n) => format!(
                    "lang item `{}` requires exactly {} generic argument(s), found {}",
                    item.key(),
                    n,
                    found
                ),
            },
        }
    }
}

impl LangItemTable {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    pub fn get(&self, item: &LangItem) -> Option<DefId> {
        self.items.get(item).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (LangItem, DefId)> + '_ {
        self.items.iter().map(|(item, def_id)| (*item, *def_id))
    }

    /// 検証なしで登録する。
    ///
    /// 依存パッケージのメタデータから復元する際に使う。
    /// そのパッケージのビルド時に検証済みであり、
    /// また AST が無いため kind / generics を再検証できない。
    pub fn set(&mut self, item: LangItem, def_id: DefId, span: Span) -> Result<(), LangItemError> {
        match self.items.entry(item) {
            Entry::Vacant(e) => {
                e.insert(def_id);
                Ok(())
            }
            Entry::Occupied(e) => Err(LangItemError::DuplicatedDefinition {
                item,
                previous: *e.get(),
                span,
            }),
        }
    }

    /// 自パッケージの定義を、種別とジェネリクス個数を検証してから登録する。
    ///
    /// rustc が回収時に `Target` と `GenericRequirement` を検証するのに相当する。
    /// 付与対象そのものの妥当性 (属性を付けてよい構文要素か) は
    /// biwac_attribute の検証パスが先に済ませている。
    pub fn set_checked(
        &mut self,
        item: LangItem,
        def_id: DefId,
        found_kind: LangItemKind,
        genarg_count: usize,
        span: Span,
    ) -> Result<(), LangItemError> {
        if item.kind() != found_kind {
            return Err(LangItemError::KindMismatch {
                item,
                found: found_kind,
                span,
            });
        }

        match item.required_generics() {
            LangItemGenericRequirement::None => {}
            LangItemGenericRequirement::Exact(n) if n == genarg_count => {}
            LangItemGenericRequirement::Exact(_) => {
                return Err(LangItemError::GenericsMismatch {
                    item,
                    found: genarg_count,
                    span,
                });
            }
        }

        self.set(item, def_id, span)
    }
}

/// lang item が型か値 (関数) かの区別。
///
/// rustc の `Target` に相当するが、biwa では現状この 2 種で足りる。
/// enum のバリアントやメソッドに lang item を付ける必要が生じたら拡張する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangItemKind {
    Ty,
    Fn,
}

impl LangItemKind {
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Ty => "a type",
            Self::Fn => "a function",
        }
    }
}

/// ジェネリック引数の個数に対する要求。
///
/// コンパイラが期待する形とライブラリ側の定義のズレを回収時に検出する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangItemGenericRequirement {
    None,
    Exact(usize),
}

macro_rules! lang_item_table {
    //
    //  ```biwa
    //  [[lang="$key"]]
    //  struct Game[C, S] { ... }
    //  ```
    //             ^^^^^
    //             $generics (LangItemGenericRequirement)
    ($($variant:ident, $key:literal, $kind:expr, $generics:expr;)*) => {
        #[repr(u32)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum LangItem {
            $($variant,)*
        }

        impl LangItem {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            pub fn kind(&self) -> LangItemKind {
                match self {
                    $(Self::$variant => $kind,)*
                }
            }

            pub fn required_generics(&self) -> LangItemGenericRequirement {
                match self {
                    $(Self::$variant => $generics,)*
                }
            }

            /// `[[lang="..."]]` に書く文字列。
            pub fn key(&self) -> &'static str {
                match self {
                    $(Self::$variant => $key,)*
                }
            }

            pub fn from_key(key: &str) -> Option<Self> {
                match key {
                    $($key => Some(Self::$variant),)*
                    _ => None,
                }
            }

            /// 依存パッケージのメタデータに書き出すための discriminant。
            ///
            /// `LangItem` を追加・並べ替えると値が変わるため、
            /// メタデータのフォーマットバージョンと歩調を合わせる必要がある。
            pub fn as_u32(&self) -> u32 {
                *self as u32
            }

            pub fn from_u32(u: u32) -> Option<Self> {
                $(
                    if u == (Self::$variant as u32) {
                        return Some(Self::$variant);
                    }
                )*

                None
            }
        }

        impl LangItemTable {
            /// 全 lang item が登録されているか検証する。
            /// no_std パッケージ (= std 自身) のビルド時に呼ぶ。
            pub fn complete(&self) -> Result<(), Vec<LangItemError>> {
                let mut errors = Vec::new();

                // FIXME:
                // after macro_metavar_expr become stable
                $(
                    if !self.items.contains_key(&LangItem::$variant) {
                        errors.push(LangItemError::MissingDefinition{ item: LangItem::$variant });
                    }
                )*

                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(errors)
                }
            }
        }
    };
}

lang_item_table!(
    // ノベルゲームのシーンを記述する scene 構文が依存する型。
    Game,      "game",      LangItemKind::Ty, LangItemGenericRequirement::Exact(2);
    Character, "character", LangItemKind::Ty, LangItemGenericRequirement::Exact(1);

    // 文字列リテラルの型。
    // コンパイラは "..." を書かれた位置でこの型を割り当てる。
    String,    "string",    LangItemKind::Ty, LangItemGenericRequirement::Exact(0);

    // エンジンへのシステムコールの記述子。
    // novel statement の展開先はこの型を返し、scene がそれを yield して
    // エンジン (kernel) に制御を渡す。中身はエンジンとの規約で、コンパイラは見ない。
    Syscall,   "syscall",   LangItemKind::Ty, LangItemGenericRequirement::Exact(0);

    // scene 内の novel statement が展開される先。
    // エンドユーザやサードパーティに直接呼ばれることを想定しておらず、
    // コンパイラのみが知っている API である。
    Write,     "write",     LangItemKind::Fn, LangItemGenericRequirement::None;
    Wait,      "wait",      LangItemKind::Fn, LangItemGenericRequirement::None;
);
