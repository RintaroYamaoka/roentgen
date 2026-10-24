pub mod ir;
pub mod ts_util;
pub mod ts;
pub mod python;
pub mod rust;
pub mod go;
pub mod sql;
pub mod config;

use crate::repo::{lang_of, Repo};
use ir::FileIR;
use rayon::prelude::*;

/// 全ソースを並列に抽出。対応外の言語は行数だけの IR。
pub fn extract_all(repo: &Repo) -> Vec<FileIR> {
    let files: Vec<&String> = repo.src_files();
    files.par_iter().map(|f| {
        let lang = lang_of(f).unwrap_or("other");
        let src = repo.read(f);
        match lang {
            "ts" | "tsx" | "js" => ts::extract(f, lang, &src),
            "py" => python::extract(f, &src),
            "rs" => rust::extract(f, &src),
            "go" => go::extract(f, &src),
            _ => FileIR { path: f.to_string(), lang: lang.into(), lines: src.lines().count() as u32, env_keys: ts_util::env_keys(&src), ..Default::default() },
        }
    }).collect()
}
