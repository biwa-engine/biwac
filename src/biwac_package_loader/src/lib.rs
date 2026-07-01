mod error;

#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub use error::PkgLoadError;

use biwac_ast::ModAst;
use biwac_base::{
    BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME, BIWA_EXTENSION, BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME,
    ErrorContext, ErrorHolder, IdentInterner, InternedIdent, MetadataHolder, ModId, ModPath,
    ModSource, PackageId, SourceHolder,
};

#[derive(Debug)]
pub struct LoadedModule {
    pub mod_id: ModId,
    pub ast: ModAst,
    pub children: HashMap<InternedIdent, LoadedModule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Lib,
    Bin,
}

#[derive(Debug)]
pub struct Pkg {
    pub pkg_kind: PackageKind,
    pub root_module: LoadedModule,
}

impl Pkg {
    // pkg_root_path はdirであることが保証されている必要がある
    pub fn try_load<'a>(
        metadata: &'a MetadataHolder,
        interner: &'a mut IdentInterner,
        srcs: &'a mut SourceHolder, // 空の SourceHolder を受け取る
        pkg_root_path: PathBuf,
    ) -> Result<Self, ErrorHolder<'a, PkgLoadError<'a>>> {
        let srcpath = pkg_root_path.join(Path::new("src"));

        let mut ctx = ModuleTreeCtx::new();

        // existence check of root module ( lib.biwa or main.biwa )
        let lib_path = {
            let lib_path = srcpath.join(Path::new(&format!(
                "{BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME}.{BIWA_EXTENSION}"
            )));
            if lib_path.exists() && lib_path.is_file() {
                Some(lib_path)
            } else {
                None
            }
        };
        let main_path = {
            let main_path = srcpath.join(Path::new(&format!(
                "{BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME}.{BIWA_EXTENSION}"
            )));
            if main_path.exists() && main_path.is_file() {
                Some(main_path)
            } else {
                None
            }
        };

        let (pkg_kind, root_mod_id, root_path) = match (lib_path, main_path) {
            (Some(path), None) => (PackageKind::Lib, ctx.alloc_mod_id(), path),
            (None, Some(path)) => (PackageKind::Bin, ctx.alloc_mod_id(), path),
            (Some(_), Some(_)) => {
                return Err(ErrorHolder {
                    errs: vec![PkgLoadError::RootModuleDuplicated],
                    ctx: ErrorContext {
                        interner,
                        srcs,
                        metadata,
                    },
                });
            }
            (None, None) => {
                return Err(ErrorHolder {
                    errs: vec![PkgLoadError::RootModuleNotFound],
                    ctx: ErrorContext {
                        interner,
                        srcs,
                        metadata,
                    },
                });
            }
        };

        let module_tree = ModuleTree {
            mod_id: root_mod_id,
            path: Box::new(root_path),
            mod_path: match pkg_kind {
                PackageKind::Lib => ModPath::Lib,
                PackageKind::Bin => ModPath::Main,
            },

            // root module を起点にモジュールツリーを構築する
            // それにはMainを指定する
            children: match map_module_tree_children_from_dir(
                &mut ctx,
                &srcpath,
                ModPath::Main,
                interner,
            ) {
                Ok(children) => children,
                Err(e) => {
                    return Err(ErrorHolder {
                        errs: vec![e],
                        ctx: ErrorContext {
                            interner,
                            srcs,
                            metadata,
                        },
                    });
                }
            },
        };

        read_module_files(srcs, &module_tree);

        let root_module = load_module(interner, srcs, module_tree);

        match root_module {
            Ok(root_module) => Ok(Self {
                pkg_kind,
                root_module,
            }),
            Err(errs) => Err(ErrorHolder {
                errs,
                ctx: ErrorContext {
                    interner,
                    srcs,
                    metadata,
                },
            }),
        }
    }
}

fn read_module_files(srcs: &mut SourceHolder, module_tree: &ModuleTree) {
    // read children module files
    for module_tree in module_tree.children.values() {
        read_module_files(srcs, module_tree);
    }

    // read self module file
    let mut f = File::open(&*module_tree.path).unwrap();
    let mut src = String::new();
    f.read_to_string(&mut src).unwrap();
    srcs.mods.insert(
        module_tree.mod_id,
        ModSource {
            modu: module_tree.mod_path.clone(),
            src,
            pkg_id: PackageId::SELF_PACKAGE,
        },
    );
}

fn load_module<'a>(
    interner: &mut IdentInterner,
    srcs: &'a SourceHolder,
    module_tree: ModuleTree,
) -> Result<LoadedModule, Vec<PkgLoadError<'a>>> {
    let mut errs = Vec::new();

    // load children modules
    let mut children = HashMap::new();
    for (interned_mod_name, module_tree) in module_tree.children {
        match load_module(interner, srcs, module_tree) {
            Ok(module) => {
                children.insert(interned_mod_name, module);
            }
            Err(e) => {
                errs.extend(e);
            }
        }
    }

    let tokens = match biwac_lexer::lex(
        interner,
        module_tree.mod_id,
        &srcs.mods.get(&module_tree.mod_id).unwrap().src,
    ) {
        Ok(tokens) => tokens,
        Err(e) => {
            errs.push(PkgLoadError::LexError {
                modpath: module_tree.mod_path.clone(),
                err: Box::new(e),
            });
            return Err(errs);
        }
    };

    let ast = biwac_parser::Parser::new(
        module_tree.mod_id,
        module_tree.mod_path.clone(),
        tokens,
        interner,
    )
    .try_parse()
    .map_err(|e| {
        errs.push(PkgLoadError::ParseError {
            modpath: module_tree.mod_path.clone(),
            err: Box::new(e),
        });
        errs
    })?;

    Ok(LoadedModule {
        mod_id: module_tree.mod_id,
        ast,
        children,
    })
}

struct ModuleTree {
    mod_id: ModId,
    mod_path: ModPath,
    path: Box<PathBuf>,
    children: HashMap<InternedIdent, ModuleTree>,
}

struct ModuleTreeCtx {
    next_mod_id: u32,
}

impl ModuleTreeCtx {
    fn new() -> Self {
        Self { next_mod_id: 0 }
    }

    fn alloc_mod_id(&mut self) -> ModId {
        let mod_id = ModId::new_in_self(self.next_mod_id); // usize→u64 cast is handled inside ModId::new
        self.next_mod_id += 1;
        mod_id
    }
}

// NOTE: `dir` must be directory path
// NOTE: call with ModPath::Main to load from top level directory
fn map_module_tree_children_from_dir<'a>(
    ctx: &mut ModuleTreeCtx,
    dir: &Path,
    modpath: ModPath,
    interner: &mut IdentInterner,
) -> Result<HashMap<InternedIdent, ModuleTree>, PkgLoadError<'a>> {
    let mut work_dir_files: HashMap<String, (ModPath, Box<PathBuf>)> = HashMap::new();
    let mut work_dir_sub_dirs: HashMap<String, Box<PathBuf>> = HashMap::new();

    for res in dir.read_dir().unwrap_or_else(|_| {
        panic!(
            "Internal error, reading directory: {}",
            dir.to_str().unwrap()
        )
    }) {
        let entry = res.expect("Internal Error, reading directory");
        let path = entry.path();

        if path.is_dir() {
            work_dir_sub_dirs.insert(
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                Box::new(path),
            );
        } else if let Some(ext) = path.extension()
            && let Some(ext_str) = ext.to_str()
            && ext_str == BIWA_EXTENSION
        {
            let file_name = path.file_stem().unwrap().to_str().unwrap().to_owned();

            // root module ではなければ登録
            // root module はトップ階層で処理
            // Mainが渡されているときは root module の意
            // matches の比較はこれで良い
            // main.biwa, lib.biwa がパッケージルートにあるが、
            // main/ や lib/ サブモジュールがあるわけではない
            if !(matches!(modpath, ModPath::Main)
                && (file_name == BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME
                    || file_name == BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME))
            {
                let mod_path = modpath.clone().push(file_name.clone());
                work_dir_files.insert(file_name, (mod_path, Box::new(path)));
            }
        }
    }

    // モジュールと同名のディレクトリがあればサブモジュールとして再帰的にロードする
    // 各種OSのファイルシステムがファイルパスの重複を許さないことを保証する限り、
    // ここで、modulesの重複を考える必要はなく、HashMap::insert()やextend()を使って良い
    work_dir_files
        .into_iter()
        .map(|(file_name, (mod_path, path))| {
            let interned = interner.get_or_insert(&file_name);
            let mod_id = ctx.alloc_mod_id();
            let children = if let Some(dir) = work_dir_sub_dirs.get(&file_name) {
                map_module_tree_children_from_dir(
                    ctx,
                    dir,
                    modpath.clone().extend(vec![file_name]),
                    interner,
                )?
            } else {
                HashMap::new()
            };

            Ok((
                interned,
                ModuleTree {
                    mod_id,
                    mod_path,
                    path,
                    children,
                },
            ))
        })
        .collect()
}
