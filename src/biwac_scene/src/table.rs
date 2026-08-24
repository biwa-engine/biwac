use std::collections::HashMap;

use biwac_span::ValDefId;

/// その scene が必須かどうか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneRequirement {
    /// playable package では必ず定義されていなければならない。
    RequiredInPlayable,

    /// 定義されていれば使われるが、無くてもよい。
    #[allow(dead_code)]
    Optional,
}

// ランタイムが名前を知っていて直接呼ぶ scene の一覧。
//
// biwac_lang_item の lang_item_table! や
// biwac_attribute の attribute_table! と同じ流儀で、
// 「どの名前が特別か」をこの表 1 箇所に集める。
//
// これらはルートモジュール (playable package なら main.biwa) に定義する。
// どのターゲット言語でどんなシンボル名になるかは codegen 側の規約であり、
// ここでは関知しない。
macro_rules! well_known_scene_table {
    ( $( $variant:ident, $name:literal, $requirement:expr ; )* ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum WellKnownScene {
            $($variant,)*
        }

        impl WellKnownScene {
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

            pub fn requirement(&self) -> SceneRequirement {
                match self {
                    $(Self::$variant => $requirement,)*
                }
            }
        }
    };
}

well_known_scene_table!(
    // ゲームのストーリー起動時にランタイムが呼ぶエントリポイント。
    // playable package では main.biwa に定義されていなければならない。
    Main, "main", SceneRequirement::RequiredInPlayable;

    // 将来ここにイベントハンドラ的なものが増える想定:
    // OnSave, "on_save", SceneRequirement::Optional;
);

/// 検査を通った既知 scene の解決結果。
///
/// library package では空になる (エントリポイントを持つのは playable package だけ)。
#[derive(Debug, Clone, Default)]
pub struct WellKnownScenes {
    scenes: HashMap<WellKnownScene, ValDefId>,
}

impl WellKnownScenes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, scene: WellKnownScene) -> Option<ValDefId> {
        self.scenes.get(&scene).copied()
    }

    /// この `ValDefId` が既知 scene のいずれかであればそれを返す。
    pub fn find(&self, def_id: &ValDefId) -> Option<WellKnownScene> {
        self.scenes
            .iter()
            .find(|(_, v)| *v == def_id)
            .map(|(k, _)| *k)
    }

    pub(crate) fn set(&mut self, scene: WellKnownScene, def_id: ValDefId) {
        self.scenes.insert(scene, def_id);
    }
}
