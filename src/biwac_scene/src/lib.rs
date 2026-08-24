mod check;
mod error;
mod table;

pub use check::check;
pub use error::{SceneError, SignatureProblem};
pub use table::{SceneRequirement, WellKnownScene, WellKnownScenes};

// この crate は「scene がランタイムとの間で守るべき規約」を持つ。
//
// - すべての scene は lang item `game` のみを引数に取り `game` を返す。
//   scene はストーリーの一区切りであり、ゲームの状態を受け取って返すためである。
// - ランタイムが名前を知っていて直接呼ぶ scene の一覧 (今は `main` だけ)。
//   playable package はエントリポイントを定義しなければならない。
//
// どのターゲット言語でどんなシンボル名として公開されるかは
// codegen 側 (biwac_generator の各 arch) の規約であり、ここでは関知しない。
