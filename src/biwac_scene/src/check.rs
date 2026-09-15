use biwac_base::{IdentInterner, ModId, PackageKind};
use biwac_hir::{FnSignature, Hir, Ty, TyKind, ValDefKind};
use biwac_lang_item::{LangItem, LangItemTable};
use biwac_span::TyDefId;

use crate::{
    SceneError, SceneRequirement, SignatureProblem, WellKnownKind, WellKnownSymbol,
    WellKnownSymbols,
};

/// scene の規約を検査し、既知 scene の解決結果を返す。
///
/// 名前解決の後・型推論の前に走らせる。
/// scene のシグネチャは名前解決の時点で確定しており、型推論を待つ必要がない。
///
/// 型 alias は lowering の最後で展開済みなので、
/// `type MyGame = Game[A, B]` 越しに書かれていても `Game` として見える。
pub fn check(
    hir: &Hir,
    lang_items: &LangItemTable,
    pkg_kind: PackageKind,
    root_mod_id: ModId,
    interner: &IdentInterner,
) -> Result<WellKnownSymbols, Vec<SceneError>> {
    let mut errors = Vec::new();
    let mut found = WellKnownSymbols::new();

    // lang item `game` が無いパッケージ (std をビルドする前など) では
    // シグネチャを照合しようがないので検査を諦める。
    // lang item の欠落自体は名前解決のパスが報告している。
    let game_ty = lang_items.get(&LangItem::Game).map(TyDefId::new);

    for (def_id, val) in &hir.vals {
        if !def_id.pkg().is_self() {
            continue;
        }

        // scene はすべて `(Game[..]) -> Game[..]` でなければならない。
        if let (ValDefKind::NovelScene(scene), Some(game_ty)) = (val, game_ty) {
            check_signature(
                &scene.signature,
                &scene.name,
                WellKnownKind::Scene,
                game_ty,
                interner,
                &mut errors,
            );
        }

        // ルートモジュールに書かれた既知の名前だけがランタイムから呼ばれる。
        let (name_ident, kind) = match val {
            ValDefKind::NovelScene(s) => (&s.name, WellKnownKind::Scene),
            ValDefKind::Fn(f) => (&f.name, WellKnownKind::Fn),
            ValDefKind::Native(f) => (&f.name, WellKnownKind::Fn),
        };
        if name_ident.span.module() != root_mod_id {
            continue;
        }
        let Some(name) = interner.get_str(&name_ident.id) else {
            continue;
        };
        let Some(well_known) = WellKnownSymbol::from_name(name) else {
            continue;
        };
        if well_known.kind() != kind {
            // 種別が違うものは登録しない。
            // 下の「必須なのに無い」の検査が理由を添えて報告する。
            continue;
        }

        // scene 以外の既知シンボルはここでシグニチャを見る。
        if kind == WellKnownKind::Fn
            && let (ValDefKind::Fn(f), Some(game_ty)) = (val, game_ty)
        {
            check_signature(&f.signature, &f.name, kind, game_ty, interner, &mut errors);
        }

        found.set(well_known, *def_id);
    }

    // エントリポイントを持つのは playable package だけ。
    // library package では既知の名前も普通の scene として扱う。
    if !pkg_kind.is_playable() {
        return finish(WellKnownSymbols::new(), errors);
    }

    for &well_known in WellKnownSymbol::ALL {
        if well_known.requirement() != SceneRequirement::RequiredInPlayable {
            continue;
        }
        if found.get(well_known).is_some() {
            continue;
        }

        // 同名の値が scene でない形で定義されていないかを見て、
        // 「無い」のか「scene でない」のかを区別して報告する。
        match val_named_with_other_kind(hir, well_known, root_mod_id, interner) {
            Some(span) => errors.push(SceneError::EntryPointNotScene {
                scene: well_known,
                span,
            }),
            None => errors.push(SceneError::MissingEntryPoint { scene: well_known }),
        }
    }

    finish(found, errors)
}

fn finish(
    found: WellKnownSymbols,
    errors: Vec<SceneError>,
) -> Result<WellKnownSymbols, Vec<SceneError>> {
    if errors.is_empty() {
        Ok(found)
    } else {
        Err(errors)
    }
}

/// ランタイムが呼ぶシンボルのシグニチャを検査する。
///
/// - scene は `(Game[..]) -> Game[..]`
/// - `on_new_game` のような関数は `() -> Game[..]`
///
/// ジェネリック引数に何が入るかは問わない。
fn check_signature(
    sig: &FnSignature,
    name_ident: &biwac_hir::Ident,
    kind: WellKnownKind,
    game_ty: TyDefId,
    interner: &IdentInterner,
    errors: &mut Vec<SceneError>,
) {
    let name = interner.get_str(&name_ident.id).unwrap_or("").to_string();

    let mut push = |reason| {
        errors.push(SceneError::InvalidSceneSignature {
            scene: name.clone(),
            kind,
            reason,
            span: name_ident.span.clone(),
        })
    };

    if sig.self_ty.is_some() {
        push(SignatureProblem::HasReceiver);
    }

    let expected = match kind {
        WellKnownKind::Scene => 1,
        WellKnownKind::Fn => 0,
    };

    match (kind, sig.args.as_slice()) {
        (WellKnownKind::Scene, [arg]) => {
            if !is_game(&arg.ty, game_ty) {
                push(SignatureProblem::ArgNotGame);
            }
        }
        (WellKnownKind::Fn, []) => {}
        (_, args) => push(SignatureProblem::ArgCount {
            found: args.len(),
            expected,
        }),
    }

    if !is_game(&sig.rty, game_ty) {
        push(SignatureProblem::ReturnNotGame);
    }
}

fn is_game(ty: &Ty, game_ty: TyDefId) -> bool {
    matches!(&ty.kind, TyKind::Defined(dt) if dt.def_id == game_ty)
}

/// ルートモジュールに同じ名前の値があるが、期待した種別でない場合にその span を返す。
///
/// 「無い」のか「種別が違う」のかを分けて報告するために使う。
fn val_named_with_other_kind(
    hir: &Hir,
    well_known: WellKnownSymbol,
    root_mod_id: ModId,
    interner: &IdentInterner,
) -> Option<biwac_span::Span> {
    hir.vals
        .iter()
        .filter(|(def_id, _)| def_id.pkg().is_self())
        .find_map(|(_, val)| {
            let (ident, kind) = match val {
                ValDefKind::Fn(f) => (&f.name, WellKnownKind::Fn),
                ValDefKind::Native(f) => (&f.name, WellKnownKind::Fn),
                ValDefKind::NovelScene(s) => (&s.name, WellKnownKind::Scene),
            };
            if kind == well_known.kind() {
                return None;
            }

            (ident.span.module() == root_mod_id
                && interner.get_str(&ident.id) == Some(well_known.name()))
            .then(|| ident.span.clone())
        })
}
