use biwac_ast::ModAst;
use biwac_base::{BiwacError, IdentInterner, ModId, ModPath};

/// 1 ファイルぶんのソーステキストを `ModAst` にする処理を差し替え可能にする。
///
/// [`crate::Pkg::try_load`] はディレクトリ木の走査・`ModId` の採番だけを
/// 担い、「1 ファイルをどう読むか」はこの trait 越しに委譲する。
/// コンパイラ本体は字句解析に回復性が無いぶん、構文エラーがあれば
/// そのファイルの読み込みごと失敗させたい ([`BiwacSourceParser`])。
/// 一方エディタ (biwa-lsp) は 1 箇所の構文エラーでパッケージ全体の
/// 名前解決が止まってほしくないので、常に (salvage された) `ModAst` を
/// 返す実装を差し込める。
pub trait SourceParser {
    fn parse<'src>(
        mod_id: ModId,
        modpath: ModPath,
        src: &'src str,
        interner: &mut IdentInterner,
    ) -> Result<ModAst, Box<dyn BiwacError + 'src>>;
}

/// コンパイラ本体の字句解析・構文解析 (`biwac_lexer` + `biwac_parser`) を
/// そのまま使う実装。構文エラーがあれば `Err` になる (回復耐性は無い)。
pub struct BiwacSourceParser;

impl SourceParser for BiwacSourceParser {
    fn parse<'src>(
        mod_id: ModId,
        modpath: ModPath,
        src: &'src str,
        interner: &mut IdentInterner,
    ) -> Result<ModAst, Box<dyn BiwacError + 'src>> {
        let tokens = biwac_lexer::lex(interner, mod_id, src)
            .map_err(|e| Box::new(e) as Box<dyn BiwacError + 'src>)?;
        biwac_parser::Parser::new(mod_id, modpath, tokens, interner)
            .try_parse()
            .map_err(|e| Box::new(e) as Box<dyn BiwacError + 'src>)
    }
}
