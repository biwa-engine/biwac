#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use biwac_ast::ModAst;
use biwac_base::{BIWA_EXTENSION, ModId, ModPath, ModSource, SourceHolder};
use biwac_lexer::TokenizeError;
use biwac_parser::ParseError;

#[derive(Debug, Clone)]
pub enum PkgLoadError {
    RootModuleNotFound,
    LexError {
        modpath: ModPath,
        err: Box<TokenizeError>,
    },
    ParseError {
        modpath: ModPath,
        err: Box<ParseError>,
    },
}

#[derive(Debug)]
pub struct Pkg {
    pub modules: HashMap<ModPath, ModAst>,
    pub srcs: SourceHolder,
}

impl Pkg {
    // pkg_root_path はdirであることが保証されている必要がある
    pub fn try_load(pkg_root_path: PathBuf) -> Result<Self, PkgLoadError> {
        let srcpath = pkg_root_path.join(Path::new("src"));

        // トップレベルモジュールを起点にロードする
        // それにはMainを指定する
        let mut file_map = FileMap::new();
        map_files_from_dir(&mut file_map, &srcpath, ModPath::Main)?;

        if !file_map.contains_lib && !file_map.contains_main {
            return Err(PkgLoadError::RootModuleNotFound);
        }

        let mut modules = HashMap::new();
        let mut srcs = HashMap::new();
        // TODO:
        // 並列実行可能
        // エラーに互いに依存がないので、複数エラーを束ねるべき
        for (file_id, (modpath, path)) in file_map.files {
            let mut f = File::open(path.as_path()).unwrap();
            let mut contents = String::new();
            f.read_to_string(&mut contents).unwrap();

            let tokens =
                biwac_lexer::lex(file_id, &contents).map_err(|e| PkgLoadError::LexError {
                    modpath: modpath.clone(),
                    err: Box::new(e),
                })?;

            let module = biwac_parser::Parser::new(tokens).try_parse().map_err(|e| {
                PkgLoadError::ParseError {
                    modpath: modpath.clone(),
                    err: Box::new(e),
                }
            })?;

            modules.insert(modpath.clone(), module);
            srcs.insert(
                file_id,
                ModSource {
                    modu: modpath,
                    src: contents,
                },
            );
        }

        Ok(Self {
            modules,
            srcs: SourceHolder { mods: srcs },
        })
    }
}

struct FileMap {
    files: HashMap<ModId, (ModPath, Box<PathBuf>)>,
    next_mod_id: usize,
    contains_lib: bool,
    contains_main: bool,
}

impl FileMap {
    fn new() -> Self {
        Self {
            files: HashMap::new(),
            next_mod_id: 0,
            contains_lib: false,
            contains_main: false,
        }
    }

    fn push(&mut self, modpath: ModPath, path: PathBuf) {
        let id = ModId::new(self.next_mod_id);
        self.next_mod_id += 1;

        if modpath == ModPath::Main {
            self.contains_main = true;
        }
        if modpath == ModPath::Lib {
            self.contains_lib = true;
        }

        self.files.insert(id, (modpath, Box::new(path)));
    }
}

// NOTE: `dir` must be directory path
// NOTE: call with ModPath::Main to load from top level directory
fn map_files_from_dir(
    pkg_file_map: &mut FileMap,
    dir: &Path,
    modpath: ModPath,
) -> Result<(), PkgLoadError> {
    let mut work_dir_sub_dirs: HashMap<String, Box<PathBuf>> = HashMap::new();
    let mut work_dir_files: HashSet<String> = HashSet::new();

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

            let modpath = if &file_name == "main" && matches!(modpath, ModPath::Main) {
                ModPath::Main
            } else if &file_name == "lib" && matches!(modpath, ModPath::Main) {
                // NOTE: とにかく、Mainが渡されているときはトップレベルモジュールの意
                // matches の比較はこれで良い
                ModPath::Lib
            } else {
                // main.biwa, lib.biwa がパッケージルートにあるが、
                // main/ や lib/ サブモジュールがあるわけではない
                work_dir_files.insert(file_name.clone());
                modpath.clone().push(file_name)
            };

            pkg_file_map.push(modpath, path);
        }
    }

    // モジュールと同名のディレクトリがあればサブモジュールとして再帰的にロードする
    // 各種OSのファイルシステムがファイルパスの重複を許さないことを保証する限り、
    // ここで、modulesの重複を考える必要はなく、HashMap::insert()やextend()を使って良い
    for file_name in work_dir_files {
        if let Some(dir) = work_dir_sub_dirs.get(&file_name) {
            map_files_from_dir(pkg_file_map, dir, modpath.clone().extend(vec![file_name]))?;
        }
    }

    Ok(())
}
