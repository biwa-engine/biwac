mod error;

#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub use error::PkgLoadError;

use biwac_ast::ModAst;
use biwac_base::{
    BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME, BIWA_EXTENSION, BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME,
    ErrorContext, ErrorHolder, IdentInterner, MetadataHolder, ModId, ModPath, ModSource,
    SourceHolder,
};

#[derive(Debug)]
pub struct LoadedModule {
    pub ast: ModAst,
    pub children: HashMap<ModId, LoadedModule>,
}

#[derive(Debug)]
pub enum PackageRootModule {
    Lib(ModId),
    Main(ModId),
}

#[derive(Debug)]
pub struct Pkg {
    pub root_mod_id: PackageRootModule,
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

        let (root_mod_id, root_path) = match (lib_path, main_path) {
            (Some(path), None) => (PackageRootModule::Lib(ctx.alloc_mod_id()), path),
            (None, Some(path)) => (PackageRootModule::Main(ctx.alloc_mod_id()), path),
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
            path: Box::new(root_path),
            mod_path: match root_mod_id {
                PackageRootModule::Lib(_) => ModPath::Lib,
                PackageRootModule::Main(_) => ModPath::Main,
            },

            // root module を起点にモジュールツリーを構築する
            // それにはMainを指定する
            children: map_module_tree_children_from_dir(&mut ctx, &srcpath, ModPath::Main)
                .map_err(|e| ErrorHolder {
                    errs: vec![e],
                    ctx: ErrorContext {
                        interner,
                        srcs,
                        metadata,
                    },
                })?,
        };

        load_module(
            interner,
            srcs,
            match root_mod_id {
                PackageRootModule::Lib(mod_id) => mod_id,
                PackageRootModule::Main(mod_id) => mod_id,
            },
            module_tree,
        )
        .map(|root_module| Self {
            root_module,
            root_mod_id,
        })
        .map_err(|errs| ErrorHolder {
            errs,
            ctx: ErrorContext {
                interner,
                srcs,
                metadata,
            },
        })
    }
}

fn load_module(
    interner: &mut IdentInterner,
    srcs: &mut SourceHolder,
    mod_id: ModId,
    module_tree: ModuleTree,
) -> Result<LoadedModule, Vec<PkgLoadError>> {
    let mut errs = Vec::new();

    // load children modules
    let children = module_tree
        .children
        .into_iter()
        .flat_map(
            |(mod_id, module_tree)| match load_module(interner, srcs, mod_id, module_tree) {
                Ok(module) => Some(module),
                Err(e) => {
                    errs.extend(e);
                    None
                }
            },
        )
        .collect();

    // load self module
    let mut f = File::open(&module_tree.path).unwrap();
    let mut src = String::new();
    f.read_to_string(&mut src).unwrap();

    let tokens = biwac_lexer::lex(interner, *mod_id, &src).map_err(|e| {
        errs.push(PkgLoadError::LexError {
            modpath: module_tree.mod_path.clone(),
            err: Box::new(e),
        });
        errs
    })?;

    let ast = biwac_parser::Parser::new(*mod_id, module_tree.mod_path.clone(), tokens)
        .try_parse()
        .map_err(|e| {
            errs.push(PkgLoadError::ParseError {
                modpath: module_tree.mod_path.clone(),
                err: Box::new(e),
            });
            errs
        })?;

    srcs.mods.insert(
        mod_id,
        ModSource {
            modu: module_tree.mod_path,
            src,
        },
    );

    Ok(LoadedModule { ast, children })
}

struct ModuleTree {
    mod_path: ModPath,
    path: Box<PathBuf>,
    children: HashMap<ModId, ModuleTree>,
}

struct ModuleTreeCtx {
    next_mod_id: usize,
}

impl ModuleTreeCtx {
    fn new() -> Self {
        Self { next_mod_id: 0 }
    }

    fn alloc_mod_id(&mut self) -> ModId {
        let mod_id = ModId::new(self.next_mod_id);
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
) -> Result<HashMap<ModId, ModuleTree>, PkgLoadError<'a>> {
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
                && (&file_name == BIWA_BINARY_PACKAGE_ROOT_MODULE_NAME
                    || &file_name == BIWA_LIBRARY_PACKAGE_ROOT_MODULE_NAME))
            {
                let mod_path = modpath.clone().push(file_name);
                work_dir_files.insert(file_name.clone(), (mod_path, Box::new(path)));
            }
        }
    }

    // モジュールと同名のディレクトリがあればサブモジュールとして再帰的にロードする
    // 各種OSのファイルシステムがファイルパスの重複を許さないことを保証する限り、
    // ここで、modulesの重複を考える必要はなく、HashMap::insert()やextend()を使って良い
    Ok(work_dir_files
        .into_iter()
        .map(|(file_name, (mod_path, path))| {
            let children = if let Some(dir) = work_dir_sub_dirs.get(&file_name) {
                map_module_tree_children_from_dir(
                    ctx,
                    dir,
                    modpath.clone().extend(vec![file_name]),
                )?
            } else {
                HashMap::new()
            };

            ModuleTree {
                mod_path,
                path,
                children,
            }
        })
        .collect())
}
