use biwac_span::Span;

use crate::WellKnownScene;

#[derive(Debug)]
pub enum SceneError {
    /// scene のシグネチャが `(Game[..]) -> Game[..]` になっていない。
    InvalidSceneSignature {
        scene: String,
        reason: SignatureProblem,
        span: Span,
    },

    /// playable package にエントリポイントが無い。
    MissingEntryPoint { scene: WellKnownScene },

    /// エントリポイントの名前は使われているが scene ではない。
    EntryPointNotScene { scene: WellKnownScene, span: Span },
}

#[derive(Debug)]
pub enum SignatureProblem {
    /// 引数の個数が 1 でない。
    ArgCount { found: usize },

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
            Self::InvalidSceneSignature { scene, reason, .. } => {
                let detail = match reason {
                    SignatureProblem::ArgCount { found } => {
                        format!("it takes {found} argument(s)")
                    }
                    SignatureProblem::ArgNotGame => "its argument is not a `Game`".to_string(),
                    SignatureProblem::ReturnNotGame => "it does not return a `Game`".to_string(),
                    SignatureProblem::HasReceiver => "it takes a receiver".to_string(),
                };

                format!(
                    "scene `{scene}` must take exactly one `Game` and return a `Game`, but {detail}"
                )
            }
            Self::MissingEntryPoint { scene } => format!(
                "this playable package has no entrypoint: define `scene {}` in the root module",
                scene.name()
            ),
            Self::EntryPointNotScene { scene, .. } => format!(
                "`{}` is the entrypoint of a playable package, so it must be a scene",
                scene.name()
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
