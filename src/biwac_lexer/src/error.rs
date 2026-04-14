use biwac_base::{BiwacError, Span};

#[derive(Debug, Clone)]
pub enum TokenizeError {
    DoubleQuoteCloseNotFound { span: Span },
}

impl BiwacError for TokenizeError {
    fn error_message(&self, srcs: &biwac_base::SourceHolder) -> String {
        match self {
            Self::DoubleQuoteCloseNotFound { span } => {
                let modsrc = srcs.mods.get(&span.module()).unwrap();

                let mut line_begin_idx = 0;
                let mut line_number = 1;
                for (i, c) in modsrc.src.char_indices() {
                    if i == span.begin() {
                        break;
                    }

                    if c == '\n' {
                        line_begin_idx = i + 1;
                        line_number += 1;
                    }
                }

                let line_src = &modsrc.src[line_begin_idx..span.begin()];
                let char_count = line_src.chars().count();

                format!(
                    r#"Error: Closing double quotation ( `"` ) expected, but not found.
  --> {}:{}:{}
 {} | {}             
    | {}{}
"#,
                    modsrc.modu.file_name(),
                    line_number,
                    char_count,
                    line_number,
                    line_src,
                    " ".repeat(char_count),
                    "^^^^"
                )
            }
        }
    }
}
