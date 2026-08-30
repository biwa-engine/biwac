//! WASM バックエンド。
//!
//! 入力は単相化済みの [`biwac_mir::MonoMir`] である。
//! ジェネリクスは消えており、到達可能な実体だけが並んでいる。

pub mod structure;
