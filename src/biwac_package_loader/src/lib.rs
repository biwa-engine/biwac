#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use biwac_ast::ModAst;
use biwac_base::{BIWA_EXTENSION, ModPath};
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
}

impl Pkg {
    pub fn try_load(rootpath: &str) -> Result<Self, PkgLoadError> {
        let root = Path::new(rootpath);

        if !root.is_dir() {
            panic!("Directory expected, but got file: `{rootpath}`");
        }

        let srcpath = root.join(Path::new("src"));

        // トップレベルモジュールを起点にロードする
        // それにはMainを指定する
        let modules = Self::try_load_from_dir(&srcpath, ModPath::Main)?.modules;

        if !modules.contains_key(&ModPath::Main) && !modules.contains_key(&ModPath::Lib) {
            return Err(PkgLoadError::RootModuleNotFound);
        }

        Ok(Self { modules })
    }

    // NOTE: `dir` must be directory path
    // NOTE: call with ModPath::Main to load from top level directory
    fn try_load_from_dir(dir: &Path, modpath: ModPath) -> Result<Self, PkgLoadError> {
        let mut dirs: HashMap<String, Box<PathBuf>> = HashMap::new();
        let mut files: HashMap<String, Box<PathBuf>> = HashMap::new();

        for res in dir.read_dir().unwrap_or_else(|_| {
            panic!(
                "Internal error, reading directory: {}",
                dir.to_str().unwrap()
            )
        }) {
            let entry = res.expect("Internal Error, reading directory");

            let path = entry.path();

            if path.is_dir() {
                dirs.insert(
                    path.file_name().unwrap().to_str().unwrap().to_owned(),
                    Box::new(path),
                );
            } else if let Some(ext) = path.extension()
                && let Some(ext_str) = ext.to_str()
                && ext_str == BIWA_EXTENSION
            {
                files.insert(
                    path.file_stem().unwrap().to_str().unwrap().to_owned(),
                    Box::new(path),
                );
            }
        }

        // .biwa ファイルをモジュールとしてロード
        // モジュールと同名のディレクトリがあればサブモジュールとして再帰的にロードする
        // 各種OSのファイルシステムがファイルパスの重複を許さないことを保証する限り、
        // ここで、modulesの重複を考える必要はなく、HashMap::insert()やextend()を使って良い
        let mut modules: HashMap<ModPath, ModAst> = HashMap::new();
        for (id, path) in files {
            let mut f = File::open(path.as_path()).unwrap();
            let mut contents = String::new();
            f.read_to_string(&mut contents).unwrap();

            let modpath = if &id == "main" && matches!(modpath, ModPath::Main) {
                ModPath::Main
            } else if &id == "lib" && matches!(modpath, ModPath::Main) {
                // NOTE: とにかく、Mainが渡されているときはトップレベルモジュールの意
                // matches の比較はこれで良い
                ModPath::Lib
            } else {
                modpath.clone().push(id.clone())
            };

            let tokens = biwac_lexer::lex(modpath.clone(), &contents).map_err(|e| {
                PkgLoadError::LexError {
                    modpath: modpath.clone(),
                    err: Box::new(e),
                }
            })?;

            let module = biwac_parser::Parser::new(tokens).try_parse().map_err(|e| {
                PkgLoadError::ParseError {
                    modpath: modpath.clone(),
                    err: Box::new(e),
                }
            })?;

            modules.insert(modpath.clone(), module);

            if let Some(dir) = dirs.get(&id) {
                let submodules = Self::try_load_from_dir(dir, modpath)?.modules;
                modules.extend(submodules);
            }
        }

        Ok(Self { modules })
    }
}
