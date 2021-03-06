//! We use code generation heavily in rust-analyzer.
//!
//! Rather then doing it via proc-macros, we use old-school way of just dumping
//! the source code.
//!
//! This module's submodules define specific bits that we generate.

mod gen_syntax;
mod gen_parser_tests;
mod gen_assists_docs;
mod gen_feature_docs;

use std::{
    fmt, mem,
    path::{Path, PathBuf},
};

use crate::{not_bash::fs2, project_root, Result};

pub use self::{
    gen_assists_docs::{generate_assists_docs, generate_assists_tests},
    gen_feature_docs::generate_feature_docs,
    gen_parser_tests::generate_parser_tests,
    gen_syntax::generate_syntax,
};

const GRAMMAR_DIR: &str = "crates/ra_parser/src/grammar";
const OK_INLINE_TESTS_DIR: &str = "crates/ra_syntax/test_data/parser/inline/ok";
const ERR_INLINE_TESTS_DIR: &str = "crates/ra_syntax/test_data/parser/inline/err";

const SYNTAX_KINDS: &str = "crates/ra_parser/src/syntax_kind/generated.rs";
const AST_NODES: &str = "crates/ra_syntax/src/ast/generated/nodes.rs";
const AST_TOKENS: &str = "crates/ra_syntax/src/ast/generated/tokens.rs";

const ASSISTS_DIR: &str = "crates/ra_assists/src/handlers";
const ASSISTS_TESTS: &str = "crates/ra_assists/src/tests/generated.rs";

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    Overwrite,
    Verify,
}

/// A helper to update file on disk if it has changed.
/// With verify = false,
fn update(path: &Path, contents: &str, mode: Mode) -> Result<()> {
    match fs2::read_to_string(path) {
        Ok(old_contents) if normalize(&old_contents) == normalize(contents) => {
            return Ok(());
        }
        _ => (),
    }
    if mode == Mode::Verify {
        anyhow::bail!("`{}` is not up-to-date", path.display());
    }
    eprintln!("updating {}", path.display());
    fs2::write(path, contents)?;
    return Ok(());

    fn normalize(s: &str) -> String {
        s.replace("\r\n", "\n")
    }
}

fn extract_comment_blocks(text: &str) -> Vec<Vec<String>> {
    do_extract_comment_blocks(text, false).into_iter().map(|(_line, block)| block).collect()
}

fn extract_comment_blocks_with_empty_lines(tag: &str, text: &str) -> Vec<CommentBlock> {
    assert!(tag.starts_with(char::is_uppercase));
    let tag = format!("{}:", tag);
    let mut res = Vec::new();
    for (line, mut block) in do_extract_comment_blocks(text, true) {
        let first = block.remove(0);
        if first.starts_with(&tag) {
            let id = first[tag.len()..].trim().to_string();
            let block = CommentBlock { id, line, contents: block };
            res.push(block);
        }
    }
    res
}

struct CommentBlock {
    id: String,
    line: usize,
    contents: Vec<String>,
}

fn do_extract_comment_blocks(
    text: &str,
    allow_blocks_with_empty_lines: bool,
) -> Vec<(usize, Vec<String>)> {
    let mut res = Vec::new();

    let prefix = "// ";
    let lines = text.lines().map(str::trim_start);

    let mut block = (0, vec![]);
    for (line_num, line) in lines.enumerate() {
        if line == "//" && allow_blocks_with_empty_lines {
            block.1.push(String::new());
            continue;
        }

        let is_comment = line.starts_with(prefix);
        if is_comment {
            block.1.push(line[prefix.len()..].to_string());
        } else {
            if !block.1.is_empty() {
                res.push(mem::take(&mut block));
            }
            block.0 = line_num + 2;
        }
    }
    if !block.1.is_empty() {
        res.push(block)
    }
    res
}

#[derive(Debug)]
struct Location {
    file: PathBuf,
    line: usize,
}

impl Location {
    fn new(file: PathBuf, line: usize) -> Self {
        Self { file, line }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.file.strip_prefix(&project_root()).unwrap().display().to_string();
        let path = path.replace('\\', "/");
        let name = self.file.file_name().unwrap();
        write!(
            f,
            "https://github.com/rust-analyzer/rust-analyzer/blob/master/{}#L{}[{}]",
            path,
            self.line,
            name.to_str().unwrap()
        )
    }
}
