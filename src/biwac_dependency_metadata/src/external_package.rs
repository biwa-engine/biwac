use std::sync::Arc;

use biwac_base::{InternedIdent, PackageId};

use crate::DepMetadata;

/// ビルド対象パッケージから見た、ロード済みの外部パッケージ 1 件。
///
/// driver が依存グラフの**推移閉包すべて**についてこれを作る。
/// 直接依存だけでは足りないのは、依存の `.biwameta` に載っているシグニチャが
/// さらにその依存の型を参照しうるからである
/// (`greeter::theme() -> color::Rgb` を `color` に依存していないパッケージが呼ぶ場合など)。
///
/// 一方で **import の根になれるのは直接依存だけ**である
/// (Rust の extern prelude と同じ)。その区別が [`Self::direct`]。
#[derive(Clone)]
pub struct ExternalPackage {
    /// パッケージ名。import のパス先頭に書ける名前でもある。
    pub ident: InternedIdent,
    /// このビルドで割り当てた ID。
    pub pkg_id: PackageId,
    pub meta: Arc<DepMetadata>,
    /// ビルド対象パッケージが直接依存しているか。
    /// 偽なら「シンボルを解決するためにロードはするが、名前では引けない」パッケージ。
    pub direct: bool,
}
