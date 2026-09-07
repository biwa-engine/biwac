use std::collections::HashMap;

use biwac_ast::{AttrBody, AttrValue, Attribute, Attrs, Globals, ImplBlock, ModAst, TypeDef};
use biwac_base::IdentInterner;
use biwac_span::Span;

use crate::{AttrError, AttrShape, AttrValueKind, KnownAttr, Target, attr_names};

/// 1 つのモジュールの AST に含まれる全定義の属性を検証する。
///
/// モジュール木の走査は呼び出し側が行う
/// (この crate は biwac_package_loader に依存しない)。
pub fn check_mod_ast(ast: &ModAst, interner: &IdentInterner, errors: &mut Vec<AttrError>) {
    for g in &ast.globals {
        match g {
            Globals::FnDef(f) => check(&f.attrs, Target::Fn, &f.id.span, interner, errors),
            Globals::NativeFnDef(f) => {
                check(&f.attrs, Target::NativeFn, &f.id.span, interner, errors)
            }
            Globals::NativeCode(c) => check(
                &c.attrs,
                Target::NativeCode,
                &c.native_span,
                interner,
                errors,
            ),
            Globals::NovelScene(s) => check(&s.attrs, Target::Scene, &s.id.span, interner, errors),
            Globals::TypeDef(t) => match t {
                TypeDef::Struct(s) => check(&s.attrs, Target::Struct, &s.id.span, interner, errors),
                TypeDef::Enum(e) => check(&e.attrs, Target::Enum, &e.id.span, interner, errors),
                TypeDef::TypeAlias(a) => {
                    check(&a.attrs, Target::TypeAlias, &a.ident.span, interner, errors)
                }
                TypeDef::NativeTypeAlias(a) => check(
                    &a.attrs,
                    Target::NativeTypeAlias,
                    &a.ident.span,
                    interner,
                    errors,
                ),
            },
            Globals::TraitDef(t) => {
                check(&t.attrs, Target::Trait, &t.id.span, interner, errors);
                for item in &t.items {
                    check(
                        &item.attrs,
                        Target::TraitItem,
                        &item.id.span,
                        interner,
                        errors,
                    );
                }
            }
            Globals::ImplBlock(b) => check_impl_block(b, interner, errors),
            Globals::Import(_) | Globals::VarDecl(_) => {}
        }
    }
}

fn check_impl_block(block: &ImplBlock, interner: &IdentInterner, errors: &mut Vec<AttrError>) {
    for f in &block.assoc_fns {
        check(&f.attrs, Target::Fn, &f.id.span, interner, errors);
    }
    for f in &block.native_assoc_fns {
        check(&f.attrs, Target::NativeFn, &f.id.span, interner, errors);
    }
    for m in &block.methods {
        check(&m.attrs, Target::Method, &m.id.span, interner, errors);
    }
    for m in &block.native_methods {
        check(&m.attrs, Target::NativeMethod, &m.id.span, interner, errors);
    }
}

/// 1 つの定義に付与された属性列を検証する。
fn check(
    attrs: &Attrs,
    target: Target,
    target_span: &Span,
    interner: &IdentInterner,
    errors: &mut Vec<AttrError>,
) {
    // 同一属性の重複検出用。属性名 -> 最初に現れた span。
    let mut seen: HashMap<KnownAttr, Span> = HashMap::new();

    for attr in attrs {
        let Some(name) = interner.get_str(&attr.name.id) else {
            continue;
        };

        let Some(known) = KnownAttr::from_name(name) else {
            errors.push(AttrError::UnknownAttribute {
                name: name.to_string(),
                span: attr.span.clone(),
            });
            continue;
        };

        if let Some(first) = seen.get(&known) {
            errors.push(AttrError::DuplicatedAttribute {
                attr: known,
                first: first.clone(),
                span: attr.span.clone(),
            });
        } else {
            seen.insert(known, attr.span.clone());
        }

        if !known.accepts(target) {
            errors.push(AttrError::InvalidTarget {
                attr: known,
                target,
                span: attr.span.clone(),
            });
            // 対象が違う場合、形状の検証結果は的外れな指摘になりやすいので止める。
            continue;
        }

        check_shape(known, attr, interner, errors);

        if known == KnownAttr::Native {
            check_arch(attr, interner, errors);
        }
    }

    // ネイティブ定義には [[native]] が付いている必要がある。
    if target.is_native() && !seen.contains_key(&KnownAttr::Native) {
        errors.push(AttrError::MissingNativeAttribute {
            target,
            span: target_span.clone(),
        });
    }
}

fn check_shape(
    known: KnownAttr,
    attr: &Attribute,
    interner: &IdentInterner,
    errors: &mut Vec<AttrError>,
) {
    let expected = known.shape();

    match (&expected, &attr.body) {
        (AttrShape::Word, AttrBody::Word) => {}

        (AttrShape::Value(kind), AttrBody::Value(val)) => {
            check_value_kind(known, None, *kind, val, errors);
        }

        (AttrShape::List(keys), AttrBody::List(args)) => {
            for arg in args {
                let Some(key) = interner.get_str(&arg.key.id) else {
                    continue;
                };

                let Some((_, expected_kind)) = keys.iter().find(|(k, _)| *k == key) else {
                    errors.push(AttrError::UnknownKey {
                        attr: known,
                        key: key.to_string(),
                        span: arg.span.clone(),
                    });
                    continue;
                };

                match (expected_kind, &arg.val) {
                    (Some(kind), Some(val)) => {
                        check_value_kind(known, Some(key), *kind, val, errors);
                    }
                    (Some(_), None) => errors.push(AttrError::MissingValue {
                        attr: known,
                        key: key.to_string(),
                        span: arg.span.clone(),
                    }),
                    (None, Some(val)) => errors.push(AttrError::UnexpectedValue {
                        attr: known,
                        key: key.to_string(),
                        span: val.span().clone(),
                    }),
                    (None, None) => {}
                }
            }
        }

        _ => errors.push(AttrError::InvalidShape {
            attr: known,
            expected,
            span: attr.span.clone(),
        }),
    }
}

fn check_value_kind(
    known: KnownAttr,
    key: Option<&str>,
    expected: AttrValueKind,
    val: &AttrValue,
    errors: &mut Vec<AttrError>,
) {
    let found = AttrValueKind::of(val);
    if found != expected {
        errors.push(AttrError::InvalidValueKind {
            attr: known,
            key: key.map(|k| k.to_string()),
            expected,
            found,
            span: val.span().clone(),
        });
    }
}

// --- 型付きアクセサ ---
//
// 検証パスを通過済みであることを前提に、値を素直に取り出す。
// 検証前に呼んだ場合は単に None が返るだけで、
// 不正な属性を正当なものとして扱ってしまうことはない。

/// `[[native(arch="...")]]` の arch を取り出す。
pub fn native_arch<'a>(attrs: &'a Attrs, interner: &IdentInterner) -> Option<(&'a str, Span)> {
    let attr = attrs.find(attr_names::Native, interner)?;
    let AttrBody::List(args) = &attr.body else {
        return None;
    };

    args.iter()
        .find(|a| interner.get_str(&a.key.id) == Some("arch"))
        .and_then(|a| a.val.as_ref())
        .and_then(|v| v.as_str().map(|s| (s, v.span().clone())))
}

/// `[[native(arch = "...")]]` の arch がコンパイラの知る名前か検証する。
///
/// 検証は [`biwac_base::Target::ALL`] に対して行う。
/// cargo feature で絞った「このビルドで生成できるターゲット」ではない。
/// `typescript` feature だけでビルドしたコンパイラでも、
/// std に書かれた `arch = "wasm"` は正当な記述でなければならないためである。
fn check_arch(attr: &Attribute, interner: &IdentInterner, errors: &mut Vec<AttrError>) {
    let AttrBody::List(args) = &attr.body else {
        return;
    };

    for arg in args {
        if interner.get_str(&arg.key.id) != Some("arch") {
            continue;
        }
        let Some(val) = &arg.val else { continue };
        let Some(name) = val.as_str() else { continue };

        if biwac_base::Target::from_name(name).is_none() {
            errors.push(AttrError::UnknownArch {
                arch: name.to_string(),
                span: val.span().clone(),
            });
        }
    }
}

/// `[[lang="..."]]` のキー文字列を取り出す。
pub fn lang_key<'a>(attrs: &'a Attrs, interner: &IdentInterner) -> Option<(&'a str, Span)> {
    let attr = attrs.find(attr_names::Lang, interner)?;
    let AttrBody::Value(val) = &attr.body else {
        return None;
    };

    val.as_str().map(|s| (s, attr.span.clone()))
}
