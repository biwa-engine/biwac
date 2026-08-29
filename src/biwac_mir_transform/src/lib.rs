//! MIR → MIR の変換。
//!
//! [`biwac_mir`] が表現、[`biwac_mir_build`](../biwac_mir_build/index.html) が HIR からの構築で、
//! ここはその上に載る「作ったあとに掛けるもの」を持つ。
//!
//! - [`simplify_cfg`] のような最適化パス。パッケージ単位で掛け、
//!   `.biwamir` に書かれる形はこれを通したあとのものになる。
//! - [`monomorphize`] — 依存も含めたプログラム全体を単相化する。
//!   こちらはパスではなく、複数パッケージの MIR を 1 つにまとめる変換である。

mod monomorphize;
mod pass;
mod simplify_cfg;

pub use monomorphize::{MonoError, MonoInput, monomorphize};
pub use pass::{MirPass, PassError, default_passes, run_passes};
pub use simplify_cfg::SimplifyCfg;
