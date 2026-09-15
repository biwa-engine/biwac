use std::collections::HashMap;

use biwac_span::ValDefId;

/// そのシンボルが必須かどうか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneRequirement {
    /// playable package では必ず定義されていなければならない。
    RequiredInPlayable,

    /// 定義されていれば使われるが、無くてもよい。
    #[allow(dead_code)]
    Optional,
}

/// ランタイムが名前を知っているシンボルの種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WellKnownKind {
    /// `scene`。`(Game[..]) -> Game[..]` で、generator として出力される。
    Scene,
    /// 普通の関数。`() -> Game[..]`。
    Fn,
}

impl WellKnownKind {
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Scene => "a scene",
            Self::Fn => "a function",
        }
    }
}

// ランタイムが名前を知っていて直接呼ぶシンボルの一覧。
//
// biwac_lang_item の lang_item_table! や
// biwac_attribute の attribute_table! と同じ流儀で、
// 「どの名前が特別か」をこの表 1 箇所に集める。
//
// これらはルートモジュール (playable package なら main.biwa) に定義する。
// どのターゲット言語でどんなシンボル名になるかは codegen 側の規約であり、
// ここでは関知しない。
macro_rules! well_known_symbol_table {
    ( $( $variant:ident, $name:literal, $kind:expr, $requirement:expr ; )* ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum WellKnownSymbol {
            $($variant,)*
        }

        impl WellKnownSymbol {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            pub fn name(&self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }

            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)*
                    _ => None,
                }
            }

            pub fn kind(&self) -> WellKnownKind {
                match self {
                    $(Self::$variant => $kind,)*
                }
            }

            pub fn requirement(&self) -> SceneRequirement {
                match self {
                    $(Self::$variant => $requirement,)*
                }
            }
        }
    };
}

well_known_symbol_table!(
    // ゲームのストーリー起動時にランタイムが呼ぶエントリポイント。
    // playable package では main.biwa に定義されていなければならない。
    Main, "main", WellKnownKind::Scene, SceneRequirement::RequiredInPlayable;

    // 最初の `Game` を組み立てる。
    //
    // `Game` は `config` や開発者定義の `characters` / `states` を含むので、
    // ランタイムには組み立てられない。wasm では `Game` が WasmGC の struct で、
    // そもそもホストから組めない。
    // したがってゲーム側が作り、ランタイムはそれを受け取って `main` に渡す。
    OnNewGame, "on_new_game", WellKnownKind::Fn, SceneRequirement::RequiredInPlayable;

    // 将来ここにイベントハンドラ的なものが増える想定:
    // OnSave, "on_save", WellKnownKind::Scene, SceneRequirement::Optional;
);

/// 検査を通った既知シンボルの解決結果。
///
/// library package では空になる (エントリポイントを持つのは playable package だけ)。
#[derive(Debug, Clone, Default)]
pub struct WellKnownSymbols {
    scenes: HashMap<WellKnownSymbol, ValDefId>,
}

impl WellKnownSymbols {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, scene: WellKnownSymbol) -> Option<ValDefId> {
        self.scenes.get(&scene).copied()
    }

    /// この `ValDefId` が既知シンボルのいずれかであればそれを返す。
    pub fn find(&self, def_id: &ValDefId) -> Option<WellKnownSymbol> {
        self.scenes
            .iter()
            .find(|(_, v)| *v == def_id)
            .map(|(k, _)| *k)
    }

    pub(crate) fn set(&mut self, scene: WellKnownSymbol, def_id: ValDefId) {
        self.scenes.insert(scene, def_id);
    }
}
