use biwac_base::{IdentInterner, ModId, PackageKind};
use biwac_hir::{FnSignature, Hir, NovelSceneDef, Ty, TyKind, ValDefKind};
use biwac_lang_item::{LangItem, LangItemTable};
use biwac_span::TyDefId;

use crate::{SceneError, SceneRequirement, SignatureProblem, WellKnownScene, WellKnownScenes};

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
) -> Result<WellKnownScenes, Vec<SceneError>> {
    let mut errors = Vec::new();
    let mut found = WellKnownScenes::new();

    // lang item `game` が無いパッケージ (std をビルドする前など) では
    // シグネチャを照合しようがないので検査を諦める。
    // lang item の欠落自体は名前解決のパスが報告している。
    let game_ty = lang_items.get(&LangItem::Game).map(TyDefId::new);

    for (def_id, val) in &hir.vals {
        if !def_id.pkg().is_self() {
            continue;
        }

        let ValDefKind::NovelScene(scene) = val else {
            continue;
        };

        if let Some(game_ty) = game_ty {
            check_signature(scene, game_ty, interner, &mut errors);
        }

        // ルートモジュールに書かれた既知の名前だけがランタイムから呼ばれる。
        if scene.name.span.module() != root_mod_id {
            continue;
        }
        let Some(name) = interner.get_str(&scene.name.id) else {
            continue;
        };
        if let Some(well_known) = WellKnownScene::from_name(name) {
            found.set(well_known, *def_id);
        }
    }

    // エントリポイントを持つのは playable package だけ。
    // library package では既知の名前も普通の scene として扱う。
    if !pkg_kind.is_playable() {
        return finish(WellKnownScenes::new(), errors);
    }

    for &well_known in WellKnownScene::ALL {
        if well_known.requirement() != SceneRequirement::RequiredInPlayable {
            continue;
        }
        if found.get(well_known).is_some() {
            continue;
        }

        // 同名の値が scene でない形で定義されていないかを見て、
        // 「無い」のか「scene でない」のかを区別して報告する。
        match non_scene_val_named(hir, well_known.name(), root_mod_id, interner) {
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
    found: WellKnownScenes,
    errors: Vec<SceneError>,
) -> Result<WellKnownScenes, Vec<SceneError>> {
    if errors.is_empty() {
        Ok(found)
    } else {
        Err(errors)
    }
}

/// scene は `(Game[..]) -> Game[..]` でなければならない。
/// ジェネリック引数に何が入るかは問わない。
fn check_signature(
    scene: &NovelSceneDef,
    game_ty: TyDefId,
    interner: &IdentInterner,
    errors: &mut Vec<SceneError>,
) {
    let sig: &FnSignature = &scene.signature;
    let name = interner.get_str(&scene.name.id).unwrap_or("").to_string();

    let mut push = |reason| {
        errors.push(SceneError::InvalidSceneSignature {
            scene: name.clone(),
            reason,
            span: scene.name.span.clone(),
        })
    };

    if sig.self_ty.is_some() {
        push(SignatureProblem::HasReceiver);
    }

    match sig.args.as_slice() {
        [arg] => {
            if !is_game(&arg.ty, game_ty) {
                push(SignatureProblem::ArgNotGame);
            }
        }
        args => push(SignatureProblem::ArgCount { found: args.len() }),
    }

    if !is_game(&sig.rty, game_ty) {
        push(SignatureProblem::ReturnNotGame);
    }
}

fn is_game(ty: &Ty, game_ty: TyDefId) -> bool {
    matches!(&ty.kind, TyKind::Defined(dt) if dt.def_id == game_ty)
}

/// ルートモジュールに `name` という名前の scene 以外の値があればその span を返す。
fn non_scene_val_named(
    hir: &Hir,
    name: &str,
    root_mod_id: ModId,
    interner: &IdentInterner,
) -> Option<biwac_span::Span> {
    hir.vals
        .iter()
        .filter(|(def_id, _)| def_id.pkg().is_self())
        .find_map(|(_, val)| {
            let ident = match val {
                ValDefKind::Fn(f) => &f.name,
                ValDefKind::Native(f) => &f.name,
                ValDefKind::NovelScene(_) => return None,
            };

            (ident.span.module() == root_mod_id && interner.get_str(&ident.id) == Some(name))
                .then(|| ident.span.clone())
        })
}
