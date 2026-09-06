//! パターンの lowering。
//!
//! 親の enum とフィールドの宣言順は解決しない。
//! 外部パッケージの enum は lowering の時点ではまだ HIR に載っていないので、
//! 自パッケージと経路を分けないために型推論に任せる
//! (`biwac_hir::VariantPattern::resolved`)。

use biwac_hir::{FieldBinding, Ident, Pattern, PatternFields, VariantPattern};
use biwac_span::DefIdKind;
use std::cell::OnceCell;

use crate::ResolveError;

use super::def_id_kind_from_path;

pub(crate) fn lower_pattern(
    pattern: &biwac_ast::Pattern,
    errors: &mut Vec<ResolveError>,
) -> Option<Pattern> {
    match pattern {
        biwac_ast::Pattern::Wildcard(span) => Some(Pattern::Wildcard(span.clone())),

        // 単独の識別子。名前解決が振り分けた結果を読む。
        biwac_ast::Pattern::Ident(p) => {
            if let Some(var_id) = p.var_id.get() {
                return Some(Pattern::Binding(*var_id, p.path.span()));
            }

            match def_id_kind_from_path(&p.path) {
                Ok(DefIdKind::Variant(variant)) => Some(Pattern::Variant(VariantPattern {
                    variant,
                    fields: PatternFields::Unit,
                    shape: biwac_ast::VariantShape::Unit,
                    span: p.path.span(),
                    resolved: OnceCell::new(),
                })),
                Ok(_) => {
                    errors.push(ResolveError::IdentNotFound {
                        ident: p.path.segments.last().unwrap().ident.clone(),
                    });
                    None
                }
                Err(e) => {
                    errors.push(e);
                    None
                }
            }
        }

        biwac_ast::Pattern::Variant(v) => {
            let variant = match def_id_kind_from_path(&v.path) {
                Ok(DefIdKind::Variant(variant)) => variant,
                Ok(_) => {
                    errors.push(ResolveError::VariantExpected {
                        path: Box::new(v.path.clone()),
                    });
                    return None;
                }
                Err(e) => {
                    errors.push(e);
                    return None;
                }
            };

            let fields = match &v.fields {
                biwac_ast::PatternFields::Unit => PatternFields::Unit,
                biwac_ast::PatternFields::Tuple(pats) => PatternFields::Positional(
                    pats.iter()
                        .filter_map(|p| lower_field_binding(p, errors))
                        .collect(),
                ),
                biwac_ast::PatternFields::Struct(fields) => PatternFields::Named(
                    fields
                        .iter()
                        .filter_map(|(name, p)| {
                            Some((Ident::from(name.clone()), lower_field_binding(p, errors)?))
                        })
                        .collect(),
                ),
            };

            Some(Pattern::Variant(VariantPattern {
                variant,
                fields,
                shape: v.fields.shape(),
                span: v.span.clone(),
                resolved: OnceCell::new(),
            }))
        }
    }
}

/// バリアントのフィールドに対する束縛。
///
/// ネストは名前解決が弾いてあるので、ここに来るのは `_` か束縛だけである。
fn lower_field_binding(
    pattern: &biwac_ast::Pattern,
    errors: &mut Vec<ResolveError>,
) -> Option<FieldBinding> {
    match pattern {
        biwac_ast::Pattern::Wildcard(span) => Some(FieldBinding::Ignore(span.clone())),
        biwac_ast::Pattern::Ident(p) => match p.var_id.get() {
            Some(var_id) => Some(FieldBinding::Bind(*var_id, p.path.span())),
            None => {
                errors.push(ResolveError::NestedPatternUnsupported {
                    span: p.path.span(),
                });
                None
            }
        },
        biwac_ast::Pattern::Variant(v) => {
            errors.push(ResolveError::NestedPatternUnsupported {
                span: v.span.clone(),
            });
            None
        }
    }
}
