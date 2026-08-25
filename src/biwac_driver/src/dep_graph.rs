use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::Path,
};

use biwac_base::PackageId;
use biwac_span::PackageHashId;

/// 依存パッケージのビルドグラフ。
///
/// - adjacency: pkg_name → 直接依存パッケージ名リスト
/// - pkg_ids: pkg_name → PackageId ((name, version) から導出した安定 id)
/// - topo_batches() で同一バッチ内の依存関係がないパッケージ群を返す
///   (将来 tokio で同一バッチを並列ビルド可能)
pub struct DepGraph {
    adjacency: HashMap<String, Vec<String>>,
    pkg_ids: HashMap<String, PackageId>,
}

impl DepGraph {
    /// ルートパッケージの直接依存リストから BFS で全推移的依存を探索し、
    /// 依存グラフを構築する。
    ///
    /// 各パッケージのマニフェストは packages_dir/<name>/biwa-package.json から読み込む。
    /// deps が空の場合は空グラフを返す。
    pub fn discover(root_deps: &[&str], packages_dir: &Path) -> Result<Self, ()> {
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut pkg_ids: HashMap<String, PackageId> = HashMap::new();
        // PackageId は 32bit に畳んだハッシュなので原理的には衝突しうる。
        // 黙って別パッケージと同一視されると極めて追いにくいので、逆引きで検査する。
        let mut id_owner: HashMap<u32, String> = HashMap::new();

        let mut queue: VecDeque<String> = root_deps.iter().map(|s| s.to_string()).collect();
        let mut visited: HashSet<String> = HashSet::new();

        while let Some(dep_name) = queue.pop_front() {
            if visited.contains(&dep_name) {
                continue;
            }
            visited.insert(dep_name.clone());

            let dep_root = packages_dir.join(&dep_name);
            let sub_meta =
                biwac_metadata_loader::try_load_package_metadata(dep_root).map_err(|e| {
                    e.print_error_message();
                })?;

            // PackageId は (name, version) から導出する。
            // 採番して記録するのではないので、誰がいつビルドしても同じ id になる。
            let pkg_id = PackageHashId::new(&sub_meta.metadata.name, &sub_meta.metadata.version)
                .as_package_id();
            if let Some(other) = id_owner.get(&pkg_id.value())
                && other != &dep_name
            {
                eprintln!(
                    "Error: package id collision between `{}` and `{}`",
                    other, dep_name
                );
                return Err(());
            }
            id_owner.insert(pkg_id.value(), dep_name.clone());
            pkg_ids.insert(dep_name.clone(), pkg_id);

            let sub_deps: Vec<String> = sub_meta
                .metadata
                .dependencies
                .iter()
                .map(|d| d.name.value().to_string())
                .collect();

            for sub_dep in &sub_deps {
                if !visited.contains(sub_dep) {
                    queue.push_back(sub_dep.clone());
                }
            }
            adjacency.insert(dep_name, sub_deps);
        }

        Ok(Self { adjacency, pkg_ids })
    }

    /// Kahn's algorithm によるトポロジカルソート。
    ///
    /// 戻り値の各バッチ (Vec<String>) 内のパッケージは互いに依存しないため、
    /// バッチ内は並列ビルド可能。バッチ間は順序を守る必要がある。
    ///
    /// 循環依存がある場合は Err(()) を返す。
    pub fn topo_batches(&self) -> Result<Vec<Vec<String>>, ()> {
        // in_degree[A] = A が依存しているパッケージのうちまだ未処理のもの数
        let mut in_degree: HashMap<&str, usize> = self
            .adjacency
            .iter()
            .map(|(k, v)| (k.as_str(), v.len()))
            .collect();

        // reverse[B] = B に依存している (= B が終われば in_degree を減らすべき) パッケージ一覧
        let mut reverse: HashMap<&str, Vec<&str>> = HashMap::new();
        for (pkg, deps) in &self.adjacency {
            for dep in deps {
                reverse.entry(dep.as_str()).or_default().push(pkg.as_str());
            }
        }

        let mut batches: Vec<Vec<String>> = Vec::new();
        let mut ready: Vec<&str> = in_degree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| *k)
            .collect();

        while !ready.is_empty() {
            batches.push(ready.iter().map(|s| s.to_string()).collect());
            let mut next_ready = Vec::new();
            for &built in &ready {
                in_degree.remove(built);
                if let Some(dependents) = reverse.get(built) {
                    for &dep in dependents {
                        if let Some(cnt) = in_degree.get_mut(dep) {
                            *cnt -= 1;
                            if *cnt == 0 {
                                next_ready.push(dep);
                            }
                        }
                    }
                }
            }
            ready = next_ready;
        }

        if in_degree.is_empty() {
            Ok(batches)
        } else {
            eprintln!("Error: circular dependency detected");
            Err(())
        }
    }

    /// グラフに含まれる全パッケージ名を、名前順にソートして返す。
    ///
    /// ルートから到達できる推移閉包そのものである。
    pub fn all_packages(&self) -> Vec<String> {
        let mut names: Vec<String> = self.adjacency.keys().cloned().collect();
        names.sort();
        names
    }

    /// パッケージの [`PackageId`]。グラフに無ければ `None`。
    pub fn pkg_id(&self, pkg_name: &str) -> Option<PackageId> {
        self.pkg_ids.get(pkg_name).copied()
    }

    /// あるパッケージから到達できる依存パッケージ名を、名前順にソートして返す
    /// (自分自身は含まない)。
    ///
    /// 鮮度判定は直接依存ではなく推移閉包を見る。
    /// パッケージのビルドは推移閉包すべてのメタデータを読む
    /// (推移的な依存が定義した lang item も取り込む) ので、
    /// 直接依存だけを見ると 2 段以上離れた変更を取りこぼす。
    pub fn transitive_deps(&self, pkg_name: &str) -> Vec<String> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<&str> = self
            .adjacency
            .get(pkg_name)
            .map(|deps| deps.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();

        while let Some(name) = queue.pop_front() {
            if !seen.insert(name) {
                continue;
            }
            if let Some(deps) = self.adjacency.get(name) {
                for d in deps {
                    if !seen.contains(d.as_str()) {
                        queue.push_back(d.as_str());
                    }
                }
            }
        }

        let mut names: Vec<String> = seen.into_iter().map(|s| s.to_string()).collect();
        names.sort();
        names
    }
}
