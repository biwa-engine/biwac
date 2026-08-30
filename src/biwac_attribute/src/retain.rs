//! 選択されていない arch の native を AST から落とす。
//!
//! std は同じ名前で arch 違いの native を並べる:
//!
//! ```biwa
//! [[native(arch="typescript")]]
//! fn write(msg: String) -> Syscall {{ ... }}
//!
//! [[native(arch="wasm")]]
//! fn write(msg: String) {{ ... }}
//! ```
//!
//! このまま名前解決に渡すと同名のシンボルが衝突する。
//! 本来は `[[feature = "..."]]` のような仕組みでシンボルの出し分けを表現すべきだが、
//! それが入るまでの間、**選択されたターゲットのものだけを残して他は落とす**。
//!
//! 落とすのは def collection より前である。
//! def collector と lowering の両方で判定すると二重管理になるので、
//! AST を刈り込んでしまい、以降のパスは
//! 「残っているのは選択された arch のものだけ」を前提にできるようにする。
//!
//! `arch` を書いていない `[[native]]` はどのターゲットでも残す。

use biwac_ast::{Globals, ImplBlock, ModAst, TypeDef};
use biwac_base::{IdentInterner, Target};

use crate::check::native_arch;

/// 選択されていない arch の native を落とす。
pub fn retain_for_target(ast: &mut ModAst, target: Target, interner: &IdentInterner) {
    ast.globals.retain_mut(|g| match g {
        Globals::NativeFnDef(f) => enabled(&f.attrs, target, interner),
        Globals::NativeCode(c) => enabled(&c.attrs, target, interner),
        Globals::TypeDef(TypeDef::NativeTypeAlias(a)) => enabled(&a.attrs, target, interner),
        Globals::ImplBlock(b) => {
            retain_in_impl_block(b, target, interner);
            true
        }
        _ => true,
    });
}

fn retain_in_impl_block(block: &mut ImplBlock, target: Target, interner: &IdentInterner) {
    block
        .native_assoc_fns
        .retain(|f| enabled(&f.attrs, target, interner));
    block
        .native_methods
        .retain(|m| enabled(&m.attrs, target, interner));
}

/// このターゲットで残すか。
///
/// `arch` を書いていなければどのターゲットでも残す。
fn enabled(attrs: &biwac_ast::Attrs, target: Target, interner: &IdentInterner) -> bool {
    match native_arch(attrs, interner) {
        Some((arch, _)) => arch == target.name(),
        None => true,
    }
}
