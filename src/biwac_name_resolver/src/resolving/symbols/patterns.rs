//! パターンの名前解決。
//!
//! パターンには 2 つの向きがある。
//!
//! - **参照**: バリアントのパスを解決する
//! - **宣言**: フィールドの束縛が新しい変数を作る
//!
//! 単独の識別子 (`None` / `x`) はどちらになるか構文からは決まらない。
//! 名前がバリアントに解決されるなら参照、されないなら宣言として扱う。
//! rustc と同じ規則である。

use biwac_ast::{IdentPattern, PathSegmentResolution, Pattern, PatternFields, VariantPattern};
use biwac_span::DefIdKind;

use crate::{
    ResolveError, ResolveErrorHandler,
    resolving::{LocalNameResolve, context::LocalResolveCtx},
};

impl<C: LocalResolveCtx> LocalNameResolve<C> for Pattern {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<ResolveError>> {
        match self {
            Pattern::Wildcard(_) => Ok(()),
            Pattern::Ident(p) => p.resolve(ctx),
            Pattern::Variant(p) => p.resolve(ctx),
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for IdentPattern {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<ResolveError>> {
        // 複製したパスで先に試す。
        // `resolve_path` は失敗しても `resolved_id` に印を書き込むので、
        // 本物のパスで試すと束縛だった場合に汚れてしまう。
        let probe = self.path.clone();
        let resolved_variant = ctx.resolve_path(&probe).is_ok()
            && matches!(
                probe.segments.last().and_then(|s| s.resolved_id.get()),
                Some(PathSegmentResolution::Ok(DefIdKind::Variant(_)))
            );

        if resolved_variant {
            // unit バリアントへの参照。本物のパスにも解決を書き込む。
            return ctx.resolve_path(&self.path).map_err(|e| vec![e]);
        }

        let ident = &self
            .path
            .segments
            .last()
            .expect("compiler bug: empty path")
            .ident;
        match ctx.declare_variable(ident) {
            Ok(var_id) => {
                self.var_id
                    .set(var_id)
                    .expect("compiler bug: pattern binding resolved twice");
                Ok(())
            }
            Err(e) => Err(vec![e]),
        }
    }
}

impl<C: LocalResolveCtx> LocalNameResolve<C> for VariantPattern {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<ResolveError>> {
        let mut errors = Vec::new();

        ctx.resolve_path(&self.path)
            .map_err(|e| vec![e])
            .handle(&mut errors);

        match &self.fields {
            PatternFields::Unit => {}
            PatternFields::Tuple(pats) => {
                for pat in pats {
                    resolve_field_pattern(pat, ctx).handle(&mut errors);
                }
            }
            PatternFields::Struct(fields) => {
                for (_, pat) in fields {
                    resolve_field_pattern(pat, ctx).handle(&mut errors);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// バリアントのフィールドに書けるパターン。
///
/// 今回はネストを入れないので、束縛か `_` だけを許す。
/// 入れ子のバリアントパターンはここで弾く。
fn resolve_field_pattern<C: LocalResolveCtx>(
    pattern: &Pattern,
    ctx: &mut C,
) -> Result<(), Vec<ResolveError>> {
    match pattern {
        Pattern::Wildcard(_) => Ok(()),
        Pattern::Ident(p) => {
            // フィールドの位置では、バリアント名であっても束縛として扱わない。
            // 入れ子は未対応なので、素直に変数を宣言する。
            let ident = &p
                .path
                .segments
                .last()
                .expect("compiler bug: empty path")
                .ident;
            match ctx.declare_variable(ident) {
                Ok(var_id) => {
                    p.var_id
                        .set(var_id)
                        .expect("compiler bug: pattern binding resolved twice");
                    Ok(())
                }
                Err(e) => Err(vec![e]),
            }
        }
        Pattern::Variant(v) => Err(vec![ResolveError::NestedPatternUnsupported {
            span: v.span.clone(),
        }]),
    }
}
