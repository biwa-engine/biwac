use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Span {
    file: PathBuf,
    begin: Pos,
    end: Pos,
}

#[derive(Clone, Debug)]
pub struct Pos {
    line: usize,
    idx: usize,
}
