use biwac_base::{PackageId, PackageName, PackageVersion};
use biwac_hash::{Hash64, StableHasher64};

/// Package stable hash generated from [`PackageName`] and [`PackageVersion`].
/// This is unique in global scope (inter-package) and inter-session.
/// Uniqueness must be ensured by depended package collector implementation.
///
/// パッケージの同一性そのものから導出するので、
/// 誰がいつビルドしても同じ値になる。
/// 「採番して記録し、次回それを引き継ぐ」方式だと、
/// その採番を持つのが誰か (ルートパッケージか、ワークスペースか) という問題が残り、
/// 同じ依存パッケージを別のルートからビルドしたときに食い違う。
///
/// rustc の `StableCrateId` (クレート名 + `-Cmetadata` + コンパイラバージョンのハッシュ) と
/// 同じ考え方である。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackageHashId(Hash64);

impl PackageHashId {
    pub fn new(pkg_name: &PackageName, pkg_version: &PackageVersion) -> Self {
        let mut hasher = StableHasher64::new();

        // 名前とバージョンの区切りが曖昧にならないよう、
        // 文字列は長さを前置して混ぜる (StableHasher64::write_str がやる)。
        hasher.write_str("biwa-package");
        hasher.write_str(pkg_name.value());
        hasher.write_usize(pkg_version.major());
        hasher.write_usize(pkg_version.minor());
        hasher.write_usize(pkg_version.patch());

        Self(hasher.finish())
    }

    pub fn as_hash(&self) -> Hash64 {
        self.0
    }

    /// ビルド中に使う 32bit の [`PackageId`] に落とす。
    ///
    /// `PackageId` の 0 (自パッケージ) と 1 (組み込み予約) は使えないので、
    /// 予約領域を避けた範囲に写す。
    ///
    /// 32bit なので原理的には衝突しうる。
    /// 依存グラフ規模 (~100) では ~1e-6 だが、黙って別パッケージと同一視されると
    /// 極めて追いにくいバグになるため、driver がグラフ全体で衝突を検査する。
    pub fn as_package_id(&self) -> PackageId {
        let span = u32::MAX - PackageId::UNRESERVED_PACKAGE_MIN;
        PackageId::new(PackageId::UNRESERVED_PACKAGE_MIN + (self.0.as_u64() as u32) % span)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn id(name: &str, version: &str) -> PackageHashId {
        PackageHashId::new(
            &PackageName::from_str(name).unwrap(),
            &PackageVersion::from_str(version).unwrap(),
        )
    }

    #[test]
    fn same_identity_same_id() {
        assert_eq!(id("std", "0.1.0"), id("std", "0.1.0"));
    }

    #[test]
    fn version_change_changes_id() {
        assert_ne!(id("std", "0.1.0"), id("std", "0.2.0"));
    }

    #[test]
    fn name_change_changes_id() {
        assert_ne!(id("std", "0.1.0"), id("color", "0.1.0"));
    }

    #[test]
    fn package_id_avoids_reserved_ids() {
        for (n, v) in [("std", "0.1.0"), ("color", "1.2.3"), ("greeter", "0.0.1")] {
            let pid = id(n, v).as_package_id();
            assert!(!pid.is_self());
            assert!(pid.value() >= PackageId::UNRESERVED_PACKAGE_MIN);
        }
    }
}
