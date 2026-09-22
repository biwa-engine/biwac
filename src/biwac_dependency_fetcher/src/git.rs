use std::path::Path;
use std::process::Command;

use crate::error::FetchError;

/// `repo_url` の `commit` を、`dest` (存在しなければ作る) に取得する。
///
/// 実装は system `git` のシェルアウト。利用者の git 設定 (ssh 鍵、
/// credential helper 等) をそのまま使えるようにするため。
/// `--depth 1` で当該コミット単体だけを取る (履歴は不要)。
pub(crate) fn fetch_commit(repo_url: &str, commit: &str, dest: &Path) -> Result<(), FetchError> {
    std::fs::create_dir_all(dest)?;

    run_git(dest, "init", &["init", "-q"])?;
    run_git(
        dest,
        "fetch",
        &["fetch", "--depth", "1", "-q", repo_url, commit],
    )?;
    run_git(dest, "checkout", &["checkout", "-q", "FETCH_HEAD"])?;

    Ok(())
}

fn run_git(dir: &Path, step: &'static str, args: &[&str]) -> Result<(), FetchError> {
    let status = Command::new("git").current_dir(dir).args(args).status()?;

    if status.success() {
        Ok(())
    } else {
        Err(FetchError::GitFailed {
            step,
            status: status.to_string(),
        })
    }
}
