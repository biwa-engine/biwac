//! HIR から MIR を構築する。
//!
//! 入力は型推論が終わった HIR で、
//! `expr_tys` / `var_tys` / `call_genargs` が埋まっていることを前提とする。
//! ここでは型を計算せず、記録されたものを引くだけである。
//!
//! 構築するのは自パッケージのシンボルだけである。
//! 依存パッケージのシンボルはシグニチャしか手元に無く、本体が無い。

mod builder;

use biwac_base::PackageId;
use biwac_hir::{AssocValDefKind, Hir, ValDefKind};
use biwac_lang_item::LangItemTable;
use biwac_mir::Mir;
use biwac_span::ValDefId;

/// 自パッケージの全シンボルを MIR に落とす。
///
/// `pkg_id` はこのパッケージの [`PackageId`]。
/// メモリ上のシンボルの `def_id.pkg()` は HIR と同じく `SELF` のままだが、
/// ディスクに書くときに要るのでここで受け取っておく。
pub fn build(hir: &Hir, lang_items: &LangItemTable, pkg_id: PackageId) -> Mir {
    let mut mir = Mir::new(hir.pkg_name.clone(), pkg_id);

    // 先にシンボルを集めて **ValDefId 順に並べてから** 落とす。
    //
    // HIR の表は HashMap なので、そのまま走査すると順序が実行ごとに変わる。
    // 文字列プールは lowering の過程で詰まっていくため、
    // 順序が変わると同じ入力から違う MIR ができてしまう。
    // ビルドの決定論は差分ビルドの前提なので、ここで固定する。
    let mut symbols: Vec<(ValDefId, Source)> = Vec::new();

    // トップレベルのシンボル。
    //
    // 依存パッケージのシンボルも hir.vals に登録されているが、
    // 本体は手元に無いので飛ばす。
    for (def_id, val) in &hir.vals {
        if !def_id.pkg().is_self() {
            continue;
        }
        symbols.push((*def_id, Source::Val(val)));
    }

    // 型に対する実装 (関連関数・メソッド)。
    //
    // 所属する型ではなく関連関数自身の ValDefId で判定する。
    // `impl Int { fn sqrt(self) }` のように、所属する型が
    // 組み込みパッケージのものであることがあるためで、
    // `.biwameta` のシンボル採番も同じ規則になっている。
    for ty_impl in hir.tys.values() {
        for impl_list in ty_impl.vals.values() {
            for (def_id, pair) in &impl_list.vals {
                if !def_id.pkg().is_self() {
                    continue;
                }
                symbols.push((*def_id, Source::Assoc(&pair.val_content)));
            }
        }
    }

    // モジュール全体に前置されるネイティブコード。
    // どのシンボルからも参照されないが、生成物の先頭に置かれる。
    // 単相化するターゲットでは依存パッケージのものも要るので、
    // `.biwamir` に載せて下流へ運ぶ。
    mir.module_natives = hir
        .module_global_natives
        .iter()
        .map(|n| n.native.clone())
        .collect();

    symbols.sort_by_key(|(def_id, _)| def_id.value());

    for (def_id, source) in symbols {
        let item = match source {
            Source::Val(ValDefKind::Fn(f)) => {
                builder::build_fn(lang_items, &mut mir.strings, def_id, f)
            }
            Source::Val(ValDefKind::NovelScene(s)) => {
                builder::build_scene(lang_items, &mut mir.strings, def_id, s)
            }
            Source::Val(ValDefKind::Native(n)) => builder::build_native_fn(def_id, n),
            Source::Assoc(AssocValDefKind::Fn(f)) => {
                builder::build_fn(lang_items, &mut mir.strings, def_id, f)
            }
            Source::Assoc(AssocValDefKind::NativeFn(n)) => builder::build_native_fn(def_id, n),
        };
        mir.items.insert(def_id, item);
    }

    mir
}

/// 落とす元になる HIR のシンボル。
enum Source<'h> {
    Val(&'h ValDefKind),
    Assoc(&'h AssocValDefKind),
}
