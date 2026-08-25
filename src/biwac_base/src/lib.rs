mod error;
mod ident;
mod module;
mod package;
mod src;

pub use error::{BiwacError, ErrorContext, ErrorHolder, print_error_finish_message};
pub use ident::{IdentInterner, InternedIdent};
pub use module::ModPath;
pub use package::{
    DependedPackage, MetadataHolder, PackageId, PackageKind, PackageMetadata, PackageName,
    PackageNameError, PackageVersion, PackageVersionError,
};
pub use src::{ModId, ModSource, SourceHolder};

pub const BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME: &str = "main";
pub const BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME: &str = "lib";
pub const BIWA_EXTENSION: &str = "biwa";

pub const BIWA_BUILD_DIRECTORY_NAME: &str = ".biwa_build";

/// 取得済みの依存パッケージが並ぶディレクトリ。
/// `<package root>/.biwa_build/deps/<package name>/` に 1 パッケージずつ入る。
///
/// **推移的依存も含めてすべてここに平らに並ぶ。**
/// 依存パッケージ自身の `.biwa_build/deps/` は見ない。
/// つまり 1 回のビルドで参照する依存ディレクトリはルートパッケージの 1 つだけである。
///
/// ここに配置するのは本来コンパイラの仕事だが、
/// 中央のパッケージハブのメタデータを引いてリポジトリから取得する必要があるため未実装。
/// 現状は「既に並んでいる」ことを前提にビルドする。
pub const BIWA_DEPENDENCIES_DIRECTORY_NAME: &str = "deps";

pub const METADATA_FILE_NAME: &str = "biwa-package.json";

/// 取得済みの依存パッケージが並ぶディレクトリのパス。
pub fn dependencies_dir(pkg_root: &std::path::Path) -> std::path::PathBuf {
    pkg_root
        .join(BIWA_BUILD_DIRECTORY_NAME)
        .join(BIWA_DEPENDENCIES_DIRECTORY_NAME)
}
