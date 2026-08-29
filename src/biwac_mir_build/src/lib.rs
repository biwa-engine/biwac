//! HIR から MIR を構築する。
//!
//! 入力は型推論が終わった HIR で、
//! `expr_tys` / `var_tys` / `call_genargs` が埋まっていることを前提とする。
//! ここでは型を計算せず、記録されたものを引くだけである。
//!
//! 構築するのは自パッケージのシンボルだけである。
//! 依存パッケージのシンボルはシグニチャしか手元に無く、本体が無い。

mod builder;
mod validate;

use biwac_hir::{AssocValDefKind, Hir, ValDefKind};
use biwac_lang_item::LangItemTable;
use biwac_mir::Mir;

pub use validate::{ValidationError, validate};

/// 自パッケージの全シンボルを MIR に落とす。
pub fn build(hir: &Hir, lang_items: &LangItemTable) -> Mir {
    let mut mir = Mir::new(hir.pkg_name.clone());

    // トップレベルのシンボル。
    //
    // 依存パッケージのシンボルも hir.vals に登録されているが、
    // 本体は手元に無いので飛ばす。
    for (def_id, val) in &hir.vals {
        if !def_id.pkg().is_self() {
            continue;
        }
        let item = match val {
            ValDefKind::Fn(f) => builder::build_fn(lang_items, &mut mir.strings, *def_id, f),
            ValDefKind::NovelScene(s) => {
                builder::build_scene(lang_items, &mut mir.strings, *def_id, s)
            }
            ValDefKind::Native(n) => builder::build_native_fn(*def_id, n),
        };
        mir.items.insert(*def_id, item);
    }

    // 型に対する実装 (関連関数・メソッド)。
    for (ty_def_id, ty_impl) in &hir.tys {
        if !ty_def_id.pkg().is_self() {
            continue;
        }
        for impl_list in ty_impl.vals.values() {
            for (def_id, pair) in &impl_list.vals {
                if !def_id.pkg().is_self() {
                    continue;
                }
                let item = match &pair.val_content {
                    AssocValDefKind::Fn(f) => {
                        builder::build_fn(lang_items, &mut mir.strings, *def_id, f)
                    }
                    AssocValDefKind::NativeFn(n) => builder::build_native_fn(*def_id, n),
                };
                mir.items.insert(*def_id, item);
            }
        }
    }

    mir
}
