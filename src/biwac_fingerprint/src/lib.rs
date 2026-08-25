//! パッケージ単位の差分ビルドの鮮度判定。
//!
//! あるパッケージについて「前回ビルドしたときと状況が変わっていないか」を判定する。
//! 変わっていなければ `.biwameta` にキャッシュされたシグニチャをそのまま使い、
//! ビルドをまるごと飛ばす。
//!
//! # 何を見るか
//!
//! 1. コンパイラの版数 — 出力の作り方が変わればキャッシュは無効
//! 2. `biwa-package.json` — 依存の追加・削除・バージョン変更、no_std の変更
//! 3. 依存グラフ (推移閉包) の各パッケージの SVH — 上流のインタフェース変更
//! 4. `src/**/*.biwa` の内容 — 自パッケージのソース変更
//!
//! 3 が推移閉包なのは、パッケージのビルドが直接依存だけでなく
//! 推移閉包すべてのメタデータを読むからである
//! (推移的な依存が定義した lang item も取り込む)。
//! 直接依存だけに絞ると、2 段以上離れたパッケージの lang item が変わったときに
//! 再ビルドされない穴ができる。
//!
//! # ソースの変更検知に mtime を使わない理由
//!
//! cargo は path 依存のパッケージについて、rustc が出す dep-info に載ったファイルの
//! mtime を成果物の mtime と比べる。速いが、
//! 「touch しただけで再ビルドされる」「mtime が保たれていて再ビルドされない」の
//! 両方の穴があり、内容ハッシュに切り替える `-Zchecksum-freshness` が別に用意されている。
//! rustc 自身の incremental compilation は逆にソースの内容ハッシュを使う。
//!
//! biwa のパッケージは数ファイル規模なので、常に内容を読んでハッシュするほうが
//! 単純で穴がない。ファイル数が増えて重くなったら `(len, mtime)` の高速パスを前置できる。

mod disk;

use std::path::{Path, PathBuf};

use biwac_base::{PackageId, PackageMetadata};
use biwac_hash::{Hash64, StableHasher64};

pub use disk::FingerprintDecodeError;

/// フィンガープリントファイルの形式版数。
///
/// 形式を変えたら上げる。
pub const BIWAC_FINGERPRINT_FORMAT_VERSION: u32 = 1;

/// 出力に影響する **コンパイラ側の変更** を表す版数。
///
/// biwac のバージョン文字列は開発中ほとんど動かないので、
/// codegen の出力の作り方を変えたらここを上げてキャッシュを一括無効化する。
/// cargo が rustc のコミットハッシュでやっていることの手動版である。
///
/// `.biwameta` の形式版数のように、別の場所で管理されている版数は
/// [`compiler_hash`] の引数として混ぜる。
pub const BIWAC_FINGERPRINT_COMPILER_EPOCH: u32 = 1;

pub const FINGERPRINT_FILE_EXTENSION: &str = "biwafp";

/// フィンガープリントファイルのパス。
pub fn fingerprint_path(build_dir: &Path, pkg_name: &str) -> PathBuf {
    build_dir.join(format!("{pkg_name}.{FINGERPRINT_FILE_EXTENSION}"))
}

/// 1 パッケージ分の、前回ビルド時の状態。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// コンパイラの同一性 (バージョン + epoch + 形式版数)
    pub compiler: Hash64,
    /// 正規化した `biwa-package.json`
    pub manifest: Hash64,
    /// 前回生成した `.biwameta` の SVH。
    /// fresh と判定したときはこれを下流へそのまま渡す。
    pub own_svh: Hash64,
    /// 依存グラフ (推移閉包) の各パッケージの SVH。id 昇順。
    pub deps: Vec<(PackageId, Hash64)>,
    /// `src/**/*.biwa` の一覧。パス昇順。
    pub sources: Vec<SourceEntry>,
}

/// ソースファイル 1 件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    /// パッケージルートからの相対パス (`/` 区切りに正規化済み)
    pub path: String,
    pub len: u64,
    pub hash: Hash64,
}

/// 鮮度の判定結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freshness {
    /// 再ビルド不要。`.biwameta` をそのまま使える。
    /// 持っているのは前回の SVH で、下流の判定にそのまま渡せる。
    Fresh(Hash64),
    /// 再ビルドが必要。
    Stale(StaleReason),
}

/// なぜ再ビルドが必要になったか。
///
/// 差分ビルドは「なぜ効かなかったか」が分からないと極めて追いにくいので、
/// 判定と同時に理由を持たせる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaleReason {
    /// 前回の成果物が無い (初回ビルド、あるいは掃除された)
    NoPreviousBuild,
    /// フィンガープリントファイルが読めない (形式変更・破損)
    UnreadableFingerprint,
    /// コンパイラが変わった
    CompilerChanged,
    /// `biwa-package.json` が変わった (依存の追加・削除・バージョン変更を含む)
    ManifestChanged,
    /// 依存のインタフェースが変わった、あるいは依存グラフの構成が変わった
    DependencyChanged,
    /// ソースファイルが変わった
    SourceChanged { path: String },
    /// ソースファイルが増減した
    SourceSetChanged,
    /// `--rebuild` が指定された
    Forced,
}

impl StaleReason {
    pub fn describe(&self) -> String {
        match self {
            Self::NoPreviousBuild => "no previous build".to_string(),
            Self::UnreadableFingerprint => "fingerprint is unreadable".to_string(),
            Self::CompilerChanged => "the compiler changed".to_string(),
            Self::ManifestChanged => "biwa-package.json changed".to_string(),
            Self::DependencyChanged => "a dependency changed".to_string(),
            Self::SourceChanged { path } => format!("`{path}` changed"),
            Self::SourceSetChanged => "the set of source files changed".to_string(),
            Self::Forced => "forced by --rebuild".to_string(),
        }
    }
}

/// コンパイラの同一性。
///
/// `extra_versions` には、キャッシュされた成果物の読み書きに関わる
/// 他の形式版数を渡す (`.biwameta` の形式版数など)。
/// これを混ぜておくと、形式を変えたときに
/// 「読めない成果物を掴んでエラーになる」のではなく
/// 「フィンガープリントが合わないので建て直す」という挙動になる。
///
/// この crate は `.biwameta` を読まないので、その版数は呼び出し側が渡す。
pub fn compiler_hash(extra_versions: &[u32]) -> Hash64 {
    let mut h = StableHasher64::new();
    h.write_str("biwac");
    h.write_str(env!("CARGO_PKG_VERSION"));
    h.write_u32(BIWAC_FINGERPRINT_COMPILER_EPOCH);
    h.write_u32(BIWAC_FINGERPRINT_FORMAT_VERSION);
    h.write_usize(extra_versions.len());
    for v in extra_versions {
        h.write_u32(*v);
    }
    h.finish()
}

/// `biwa-package.json` の内容のうち、ビルド結果に影響する部分のハッシュ。
///
/// 生テキストではなくパース済みのフィールドから作るので、
/// 整形やキーの並べ替え、コメントの追加では変わらない。
pub fn manifest_hash(metadata: &PackageMetadata) -> Hash64 {
    let mut h = StableHasher64::new();
    h.write_str("biwa-manifest");
    h.write_str(metadata.name.value());
    h.write_usize(metadata.version.major());
    h.write_usize(metadata.version.minor());
    h.write_usize(metadata.version.patch());
    h.write_u8(metadata.no_std as u8);

    // dependencies の記載順で再ビルドさせないようソートする。
    let mut deps: Vec<String> = metadata
        .dependencies
        .iter()
        .map(|d| {
            let max = d
                .max_version
                .as_ref()
                .map(|v| format!("{}.{}.{}", v.major(), v.minor(), v.patch()))
                .unwrap_or_else(|| "*".to_string());
            format!(
                "{}:{}.{}.{}:{}",
                d.name.value(),
                d.min_version.major(),
                d.min_version.minor(),
                d.min_version.patch(),
                max
            )
        })
        .collect();
    deps.sort();

    h.write_usize(deps.len());
    for d in &deps {
        h.write_str(d);
    }

    h.finish()
}

/// `src/` 以下の `.biwa` を列挙して内容ハッシュを取る。
///
/// biwa のモジュール木はディレクトリ走査で決まる (`mod` 宣言が無い) ので、
/// 「パッケージのソース」= `src/**/*.biwa` そのものである。
/// ファイルが増えるだけでもモジュール木が変わるため、集合の変化も検知対象になる。
pub fn collect_sources(pkg_root: &Path) -> std::io::Result<Vec<SourceEntry>> {
    let src_dir = pkg_root.join("src");
    let mut out = Vec::new();
    collect_sources_in(&src_dir, &src_dir, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_sources_in(root: &Path, dir: &Path, out: &mut Vec<SourceEntry>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_sources_in(root, &path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some(biwac_base::BIWA_EXTENSION) {
            let bytes = std::fs::read(&path)?;
            let mut h = StableHasher64::new();
            h.write_bytes(&bytes);

            let rel = path.strip_prefix(root).unwrap_or(&path);
            // OS 間で区切り文字が変わるので `/` に正規化する。
            let rel = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");

            out.push(SourceEntry {
                path: rel,
                len: bytes.len() as u64,
                hash: h.finish(),
            });
        }
    }
    Ok(())
}

impl Fingerprint {
    /// 今回のビルドの状態からフィンガープリントを作る。
    pub fn of_build(
        compiler: Hash64,
        metadata: &PackageMetadata,
        own_svh: Hash64,
        deps: &[(PackageId, Hash64)],
        sources: Vec<SourceEntry>,
    ) -> Self {
        let mut deps = deps.to_vec();
        deps.sort_by_key(|(pkg_id, _)| pkg_id.value());

        Self {
            compiler,
            manifest: manifest_hash(metadata),
            own_svh,
            deps,
            sources,
        }
    }

    /// 前回のフィンガープリント (`self`) と今回の状況を突き合わせる。
    ///
    /// `sources` は今回集めたもの。
    /// 一致すれば前回の SVH を返し、そのパッケージのビルドを飛ばせる。
    pub fn freshness(
        &self,
        compiler: Hash64,
        metadata: &PackageMetadata,
        deps: &[(PackageId, Hash64)],
        sources: &[SourceEntry],
    ) -> Freshness {
        if self.compiler != compiler {
            return Freshness::Stale(StaleReason::CompilerChanged);
        }
        if self.manifest != manifest_hash(metadata) {
            return Freshness::Stale(StaleReason::ManifestChanged);
        }

        let mut deps = deps.to_vec();
        deps.sort_by_key(|(pkg_id, _)| pkg_id.value());
        if self.deps != deps {
            return Freshness::Stale(StaleReason::DependencyChanged);
        }

        if self.sources.len() != sources.len() {
            return Freshness::Stale(StaleReason::SourceSetChanged);
        }
        for (before, now) in self.sources.iter().zip(sources) {
            if before.path != now.path {
                return Freshness::Stale(StaleReason::SourceSetChanged);
            }
            if before.len != now.len || before.hash != now.hash {
                return Freshness::Stale(StaleReason::SourceChanged {
                    path: now.path.clone(),
                });
            }
        }

        Freshness::Fresh(self.own_svh)
    }
}
