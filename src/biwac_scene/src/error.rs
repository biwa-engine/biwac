use biwac_span::Span;

use crate::WellKnownSymbol;

#[derive(Debug)]
pub enum SceneError {
    /// ランタイムが呼ぶシンボルのシグネチャが期待と違う。
    ///
    /// scene は `(Game[..]) -> Game[..]`、
    /// `on_new_game` のような関数は `() -> Game[..]` である。
    InvalidSceneSignature {
        scene: String,
        kind: crate::WellKnownKind,
        reason: SignatureProblem,
        span: Span,
    },

    /// playable package に必須のシンボルが無い。
    MissingEntryPoint { scene: WellKnownSymbol },

    /// 名前は使われているが、期待した種別 (scene / 関数) ではない。
    EntryPointNotScene { scene: WellKnownSymbol, span: Span },
}

#[derive(Debug)]
pub enum SignatureProblem {
    /// 引数の個数が期待と違う。
    ArgCount { found: usize, expected: usize },

    /// 引数の型が lang item `game` ではない。
    ArgNotGame,

    /// 戻り値の型が lang item `game` ではない。
    ReturnNotGame,

    /// レシーバ (`self`) を取っている。
    HasReceiver,
}

impl SceneError {
    pub fn span(&self) -> Option<&Span> {
        match self {
            Self::InvalidSceneSignature { span, .. } | Self::EntryPointNotScene { span, .. } => {
                Some(span)
            }
            Self::MissingEntryPoint { .. } => None,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidSceneSignature {
                scene,
                kind,
                reason,
                ..
            } => {
                let detail = match reason {
                    SignatureProblem::ArgCount { found, expected } => {
                        format!("it takes {found} argument(s) instead of {expected}")
                    }
                    SignatureProblem::ArgNotGame => "its argument is not a `Game`".to_string(),
                    SignatureProblem::ReturnNotGame => "it does not return a `Game`".to_string(),
                    SignatureProblem::HasReceiver => "it takes a receiver".to_string(),
                };

                let expected = match kind {
                    crate::WellKnownKind::Scene => "take exactly one `Game` and return a `Game`",
                    crate::WellKnownKind::Fn => "take no argument and return a `Game`",
                };

                format!("`{scene}` must {expected}, but {detail}")
            }
            Self::MissingEntryPoint { scene } => format!(
                "this playable package must define {} `{}` in the root module",
                scene.kind().describe(),
                scene.name()
            ),
            Self::EntryPointNotScene { scene, .. } => format!(
                "the runtime calls `{}` directly, so it must be {}",
                scene.name(),
                scene.kind().describe()
            ),
        }
    }
}

impl biwac_base::BiwacError for SceneError {
    fn print_error_message(&self, _ctx: &biwac_base::ErrorContext) {
        // TODO: 他のエラー系 crate と同様、ariadne による
        //       ソース抜粋付きの診断表示は未実装。
        eprintln!("Error: {}", self.message());
    }
}
