//! WASM バックエンド。
//!
//! 入力は単相化済みの [`biwac_mir::MonoMir`] である。
//! ジェネリクスは消えており、到達可能な実体だけが並んでいる。

pub mod emit;
pub mod structure;

pub use emit::{WasmError, emit};

/// WAT を wasm バイナリにする。
///
/// アセンブルしたあと検証まで通す。
/// `wat` は「符号化できるか」しか見ないので、
/// 型が合っていない出力を黙って通してしまう。
/// 壊れた wasm を書き出して実行時に初めて気づくより、ここで止めるほうがよい。
pub fn assemble(wat: &str) -> Result<Vec<u8>, String> {
    let binary = wat::parse_str(wat).map_err(|e| e.to_string())?;

    // GC / 参照型を使うので、既定より広い機能集合で検証する。
    let mut validator = wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all());
    validator
        .validate_all(&binary)
        .map_err(|e| format!("the generated wasm is invalid: {e}"))?;

    Ok(binary)
}
