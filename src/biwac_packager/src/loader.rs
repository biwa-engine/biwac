use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use biwac_base::ModPath;
use biwac_parser::ModAst;

use crate::resolver::{AbsId, symbols::ModSym, try_resolve_imports};

const SOURCECODE_ROOT_MODULE: &str = "main";
const SOURCECODE_EXTENSION: &str = "code";

#[derive(Debug, Clone)]
pub enum PkgLoadError {
    RootModuleNotFound,
}

#[derive(Debug)]
pub(crate) struct PkgSymMap {
    syms: HashMap<AbsId, ModSym>,
}

impl PkgSymMap {
    pub(crate) fn syms(&self) -> &HashMap<AbsId, ModSym> {
        &self.syms
    }

    pub(super) fn load(rootpath: &str) -> Result<Self, PkgLoadError> {
        let root = Path::new(rootpath);

        if !root.is_dir() {
            panic!("Directory Expected, But Got File: `{rootpath}`");
        }

        let srcpath = root.join(Path::new("src"));

        let toplevel_modules = Self::load_from_dir(&srcpath, ModPath::Main)?.syms;

        if !toplevel_modules.contains_key(&AbsId::new(vec![], SOURCECODE_ROOT_MODULE.to_string())) {
            panic!("Root Module `{SOURCECODE_ROOT_MODULE}` Not Found");
        }

        Ok(Self {
            syms: toplevel_modules,
        })
    }

    // NOTE: `dir` must be directory path
    fn load_from_dir(dir: &Path, modpath: ModPath) -> Result<Self, PkgLoadError> {
        let mut dirs: HashMap<String, Box<PathBuf>> = HashMap::new();
        let mut files: HashMap<String, Box<PathBuf>> = HashMap::new();

        for res in dir.read_dir().unwrap_or_else(|_| {
            panic!(
                "Internal Error, Reading Directory: {}",
                dir.to_str().unwrap()
            )
        }) {
            let entry = res.expect("Internal Error, Reading Directory");

            let path = entry.path();

            if path.is_dir() {
                dirs.insert(
                    path.file_name().unwrap().to_str().unwrap().to_owned(),
                    Box::new(path),
                );
            } else if let Some(ext) = path.extension() {
                if let Some(ext_str) = ext.to_str() {
                    if ext_str == SOURCECODE_EXTENSION {
                        files.insert(
                            path.file_stem().unwrap().to_str().unwrap().to_owned(),
                            Box::new(path),
                        );
                    }
                }
            }
        }

        if !files.contains_key(SOURCECODE_ROOT_MODULE) {
            return Err(PkgLoadError::RootModuleNotFound);
        }

        let mut syms: HashMap<AbsId, ModSym> = HashMap::new();
        for (id, path) in files {
            let mut f = File::open(path.as_path()).unwrap();
            let mut contents = String::new();
            f.read_to_string(&mut contents).unwrap();
            let modpath = if &id == "main" {
                modpath.clone()
            } else {
                modpath.clone().push(id.clone())
            };

            let tokens = biwac_lexer::lex(modpath.clone(), &contents).expect("Tokenize Error");

            match ModAst::try_parse(tokens) {
                Ok(prog) => match try_resolve_imports(prog, &modpath) {
                    Ok(module) => {
                        syms.extend(module);
                    }
                    Err(e) => {
                        panic!("Resolve Error: {e:?}");
                    }
                },
                Err(e) => {
                    panic!("Resolve Error: {e:?}");
                    // e.panic_with_error_message(&contents);
                }
            }

            let children = if let Some(dir) = dirs.get(&id) {
                Self::load_from_dir(dir, modpath.clone().push(id.to_owned()))?.syms
            } else {
                HashMap::new()
            };

            syms.extend(children);
        }

        Ok(Self { syms })
    }
}
