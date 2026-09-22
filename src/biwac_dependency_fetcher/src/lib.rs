//! 依存パッケージが `.biwa_build/deps/<name>/` に無いとき、
//! Biwa Package Hub から解決して git 越しに取得する。
//!
//! アルゴリズムの詳細は `tools/hub/README.md` の「自動フェッチのアルゴリズム」を参照。

mod error;
mod git;

use std::path::Path;
use std::str::FromStr;

use biwac_base::PackageVersion;
use biwa_hub_client::HubClient;

pub use error::FetchError;

pub struct Fetcher {
    hub: HubClient,
}

impl Fetcher {
    /// ビルド時に取り込まれた hub の URL (`BIWA_HUB_URL`) を使う。
    /// 未設定なら [`FetchError::Hub`] を返す。
    pub fn new() -> Result<Self, FetchError> {
        Ok(Self {
            hub: HubClient::new()?,
        })
    }

    /// `name` の、`min..max` を満たす最新バージョンを `dest` に取得する。
    ///
    /// 取得したバージョン自体の依存は見ない — 取得先の `biwa-package.json` を
    /// 呼び出し元 (`biwac_driver::DepGraph::discover`) がいつも通り読み直すことで、
    /// 直接依存を解決するのと同じ経路に合流する。
    pub fn fetch(
        &self,
        name: &str,
        min: &PackageVersion,
        max: Option<&PackageVersion>,
        dest: &Path,
    ) -> Result<(), FetchError> {
        let package = self.hub.get_package_by_name(name)?;
        let versions = self.hub.list_versions(name)?;

        let picked = versions
            .into_iter()
            .filter_map(|v| PackageVersion::from_str(&v.version).ok().map(|ver| (ver, v)))
            .filter(|(ver, _)| ver.in_range(min, max))
            .max_by_key(|(ver, _)| *ver)
            .map(|(_, v)| v)
            .ok_or_else(|| FetchError::NoMatchingVersion {
                name: name.to_string(),
                min: min.to_string(),
                max: max.map(|m| m.to_string()),
            })?;

        println!(
            "Fetching `{name}` v{} from {} @ {}",
            picked.version, package.repository, picked.commit
        );

        git::fetch_commit(&package.repository, &picked.commit, dest)
    }
}
