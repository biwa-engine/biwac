pub mod arch;

use biwac_base::Target;

/// このビルドのコンパイラが生成できるターゲットの一覧。
///
/// [`Target::ALL`] が「存在するターゲット」であるのに対し、
/// こちらは cargo feature で絞った「作れるターゲット」である。
/// どの feature がどのバックエンドを引くかを知っているのはこの crate なので、
/// ここに置いてある。
pub fn available_targets() -> Vec<Target> {
    let mut targets = Vec::new();
    if cfg!(feature = "typescript") {
        targets.push(Target::TypeScript);
    }
    if cfg!(feature = "wasm") {
        targets.push(Target::Wasm);
    }
    targets
}
