use biwac_ast::{Attrs, Globals, ImplBlock, ModAst, TypeDef};
use biwac_base::IdentInterner;
use biwac_dependency_metadata::ExternalPackage;
use biwac_lang_item::{LangItem, LangItemKind, LangItemTable};
use biwac_package_loader::Pkg;
use biwac_span::{DefId, Span};

use crate::ResolveError;

// lang item の回収。
//
// DefCollector とは独立したパスである。
// DefId は AST ノードの OnceCell に格納済みなので、
// def collection の後に AST を再走査すれば
// 属性と DefId を同じノードから読める。
// (rustc の LanguageItemCollector が独立したビジタであるのと同じ構造)
//
// 属性の形状 (`[[lang="..."]]` が文字列値を 1 つ取ること、
// 付与できる構文要素であること) は
// biwac_attribute の検証パスが先に済ませている。
// ここではキーの妥当性と、種別・ジェネリクス個数の整合を見る。

/// 依存パッケージのメタデータと自パッケージの AST から lang item テーブルを構築する。
///
/// 依存 → 自パッケージの順に登録するため、
/// ユーザパッケージが依存の lang item を再定義すると
/// `DuplicatedDefinition` になる。
/// rustc が `LangItem` と `DefId` の全単射を fatal error で守るのと同じ効果で、
/// 「ユーザに lang item を定義させない」ための追加の防御は要らない。
pub(crate) fn collect_lang_items(
    pkg: &Pkg,
    external_packages: &[ExternalPackage],
    no_std: bool,
    interner: &IdentInterner,
) -> Result<LangItemTable, Vec<ResolveError>> {
    let mut table = LangItemTable::new();
    let mut errors = Vec::new();

    // 依存パッケージが定義した lang item を先に取り込む。
    //
    // 直接依存に限らず推移閉包すべてを見る。
    // 各パッケージの .biwameta には自分が定義した lang item しか載らないので
    // (DepMetadata::new が is_self() で絞っている)、重複登録にはならない。
    for p in external_packages {
        let (pkg_id, dep) = (p.pkg_id, &p.meta);
        for (item, def_id) in dep.lang_items(pkg_id) {
            // 依存側は自身のビルド時に検証済みであり、
            // またここには AST が無いため種別・ジェネリクスは再検証できない。
            if let Err(e) = table.set(item, def_id, Span::dummy()) {
                errors.push(ResolveError::LangItem(e));
            }
        }
    }

    // 自パッケージの定義を重ねる。
    pkg.walk_modules(|module| {
        collect_in_mod_ast(&module.ast, &mut table, interner, &mut errors);
    });

    // no_std パッケージ (= std 自身) は全 lang item の提供元なので、
    // 揃っていることをここで検証する。
    // それ以外のパッケージでは依存から復元済みのはずであり、
    // 欠けていた場合は使用箇所で型推論エラーになる。
    if no_std && errors.is_empty() {
        if let Err(errs) = table.complete() {
            errors.extend(errs.into_iter().map(ResolveError::LangItem));
        }
    }

    if errors.is_empty() {
        Ok(table)
    } else {
        Err(errors)
    }
}

fn collect_in_mod_ast(
    ast: &ModAst,
    table: &mut LangItemTable,
    interner: &IdentInterner,
    errors: &mut Vec<ResolveError>,
) {
    for g in &ast.globals {
        match g {
            Globals::FnDef(f) => {
                register(table, &f.attrs, interner, errors, || {
                    Some((
                        f.def_id.get()?.def_id(),
                        LangItemKind::Fn,
                        genarg_count(f.genargs.as_ref().map(|g| g.genargs.len())),
                    ))
                });
            }
            Globals::NativeFnDef(f) => {
                register(table, &f.attrs, interner, errors, || {
                    Some((
                        f.def_id.get()?.def_id(),
                        LangItemKind::Fn,
                        genarg_count(f.genargs.as_ref().map(|g| g.genargs.len())),
                    ))
                });
            }
            Globals::TypeDef(t) => match t {
                TypeDef::Struct(s) => {
                    register(table, &s.attrs, interner, errors, || {
                        Some((
                            s.def_id.get()?.def_id(),
                            LangItemKind::Ty,
                            genarg_count(s.genargs.as_ref().map(|g| g.genargs.len())),
                        ))
                    });
                }
                TypeDef::Enum(e) => {
                    register(table, &e.attrs, interner, errors, || {
                        Some((
                            e.def_id.get()?.def_id(),
                            LangItemKind::Ty,
                            genarg_count(e.genargs.as_ref().map(|g| g.genargs.len())),
                        ))
                    });
                }
                TypeDef::TypeAlias(a) => {
                    register(table, &a.attrs, interner, errors, || {
                        Some((
                            a.def_id.get()?.def_id(),
                            LangItemKind::Ty,
                            genarg_count(a.genargs.as_ref().map(|g| g.genargs.len())),
                        ))
                    });
                }
                TypeDef::NativeTypeAlias(a) => {
                    register(table, &a.attrs, interner, errors, || {
                        Some((
                            a.def_id.get()?.def_id(),
                            LangItemKind::Ty,
                            genarg_count(a.genargs.as_ref().map(|g| g.genargs.len())),
                        ))
                    });
                }
            },
            Globals::ImplBlock(b) => collect_in_impl_block(b, table, interner, errors),
            // trait への lang item はまだ扱わない。
            // 演算子オーバーロードを入れるときに要る。
            Globals::TraitDef(_)
            | Globals::NovelScene(_)
            | Globals::NativeCode(_)
            | Globals::Import(_)
            | Globals::VarDecl(_) => {}
        }
    }
}

fn collect_in_impl_block(
    block: &ImplBlock,
    table: &mut LangItemTable,
    interner: &IdentInterner,
    errors: &mut Vec<ResolveError>,
) {
    for f in &block.assoc_fns {
        register(table, &f.attrs, interner, errors, || {
            Some((
                f.def_id.get()?.def_id(),
                LangItemKind::Fn,
                genarg_count(f.genargs.as_ref().map(|g| g.genargs.len())),
            ))
        });
    }
    for f in &block.native_assoc_fns {
        register(table, &f.attrs, interner, errors, || {
            Some((
                f.def_id.get()?.def_id(),
                LangItemKind::Fn,
                genarg_count(f.genargs.as_ref().map(|g| g.genargs.len())),
            ))
        });
    }
    for m in &block.methods {
        register(table, &m.attrs, interner, errors, || {
            Some((
                m.def_id.get()?.def_id(),
                LangItemKind::Fn,
                genarg_count(m.genargs.as_ref().map(|g| g.genargs.len())),
            ))
        });
    }
    for m in &block.native_methods {
        register(table, &m.attrs, interner, errors, || {
            Some((
                m.def_id.get()?.def_id(),
                LangItemKind::Fn,
                genarg_count(m.genargs.as_ref().map(|g| g.genargs.len())),
            ))
        });
    }
}

fn genarg_count(opt: Option<usize>) -> usize {
    opt.unwrap_or(0)
}

/// `[[lang="..."]]` が付いていればテーブルに登録する。
///
/// DefId の取得を遅延させるため、定義側の情報はクロージャで受け取る。
/// 属性が付いていない定義に対して OnceCell を覗く無駄を避ける。
fn register(
    table: &mut LangItemTable,
    attrs: &Attrs,
    interner: &IdentInterner,
    errors: &mut Vec<ResolveError>,
    def: impl FnOnce() -> Option<(DefId, LangItemKind, usize)>,
) {
    let Some((key, span)) = biwac_attribute::lang_key(attrs, interner) else {
        return;
    };

    let Some(item) = LangItem::from_key(key) else {
        errors.push(ResolveError::LangItem(
            biwac_lang_item::LangItemError::UnknownKey {
                key: key.to_string(),
                span,
            },
        ));
        return;
    };

    // DefId が未設定なのは def collection が失敗した場合のみで、
    // その場合は既に別のエラーが報告されている。
    let Some((def_id, kind, genargs)) = def() else {
        return;
    };

    if let Err(e) = table.set_checked(item, def_id, kind, genargs, span) {
        errors.push(ResolveError::LangItem(e));
    }
}
