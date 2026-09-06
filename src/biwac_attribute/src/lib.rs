mod check;
mod error;
mod retain;
mod table;

pub use check::{check_mod_ast, lang_key, native_arch};
pub use error::AttrError;
pub use retain::retain_for_target;
pub use table::{AttrShape, AttrValueKind, KnownAttr, attr_names};

// この crate は「どの属性が存在し、何を取り、どこに付けられるか」の
// 唯一の情報源である。
//
// biwac_ast は属性の構文表現のみを持ち (何が書かれていたか)、
// この crate が一覧と検証を持つ (何が正しいか)。
//
// 検証は AST から HIR への lowering までの間に走るパスであり、
// def collection より前に通す。
// これにより後段の lang item 回収パスは
// 「属性の形状は既に正当」という前提で書ける。

/// 属性を付与できる対象の種別。
///
/// rustc の `Target` に相当する。
/// parser が `[[native]] fn` を `NativeFnDef` として構築するため、
/// 検証時点の AST ノード種別がそのまま対象種別になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Fn,
    NativeFn,
    Method,
    NativeMethod,
    Struct,
    Enum,
    TypeAlias,
    NativeTypeAlias,
    NativeCode,
    Scene,
}

impl Target {
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Fn => "a function",
            Self::NativeFn => "a native function",
            Self::Method => "a method",
            Self::NativeMethod => "a native method",
            Self::Struct => "a struct",
            Self::Enum => "an enum",
            Self::TypeAlias => "a type alias",
            Self::NativeTypeAlias => "a native type alias",
            Self::NativeCode => "a native code block",
            Self::Scene => "a scene",
        }
    }

    /// ネイティブ実装を伴う対象か。
    /// これらには `[[native]]` が必ず付いている必要がある。
    pub fn is_native(&self) -> bool {
        matches!(
            self,
            Self::NativeFn | Self::NativeMethod | Self::NativeTypeAlias | Self::NativeCode
        )
    }
}
