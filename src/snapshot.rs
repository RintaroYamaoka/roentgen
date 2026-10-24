//! スナップショット(~/.roentgen/<repo>/<date-sha>/data)と出力の既定の置き場。
use std::path::{Path, PathBuf};

pub fn home() -> PathBuf { std::env::var("ROENTGEN_HOME").map(PathBuf::from).unwrap_or_else(|_| dirs_home().join(".roentgen")) }
fn dirs_home() -> PathBuf { std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(".")) }

pub fn repo_key(root: &Path) -> String {
    let remote = crate::repo::sh(&["git", "remote", "get-url", "origin"], Some(root));
    let rx = regex::Regex::new(r"[:/]([^/]+/[^/]+?)(?:\.git)?\s*$").unwrap();
    rx.captures(remote.trim()).map(|m| m[1].replace('/', "__")).unwrap_or_else(|| root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or("repo".into()))
}

pub fn save(root: &Path, data_dir: &Path, analyzed: &str, sha: &str) -> anyhow::Result<PathBuf> {
    let dst = home().join(repo_key(root)).join(format!("{}-{}", analyzed, sha));
    if dst.exists() { std::fs::remove_dir_all(&dst)?; }
    copy_dir(data_dir, &dst.join("data"))?;
    std::fs::write(home().join(repo_key(root)).join("last"), dst.file_name().unwrap().to_string_lossy().as_bytes())?;
    Ok(dst)
}

pub fn previous(root: &Path, current: &str) -> Option<PathBuf> {
    let d = home().join(repo_key(root));
    let mut names: Vec<String> = std::fs::read_dir(&d).ok()?.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    names.sort();
    let others: Vec<&String> = names.iter().filter(|n| n.as_str() != current).collect();
    if let Some(n) = others.last() { return Some(d.join(n)); }
    if names.iter().any(|n| n == current) { return Some(d.join(current)); }
    None
}

pub fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? { let e = e?; let p = e.path(); let t = dst.join(e.file_name()); if p.is_dir() { copy_dir(&p, &t)?; } else { std::fs::copy(&p, &t)?; } }
    Ok(())
}

/// 指定 sha を一時 worktree に出して解析する。
pub fn worktree_at(root: &Path, sha: &str) -> Option<(PathBuf, PathBuf)> {
    let tmp = std::env::temp_dir().join(format!("roentgen-since-{}", std::process::id()));
    let wt = tmp.join("wt");
    let _ = std::fs::create_dir_all(&tmp);
    let o = std::process::Command::new("git").args(["worktree", "add", "--detach", wt.to_str()?, sha]).current_dir(root).output().ok()?;
    if !o.status.success() || !wt.exists() { let _ = std::fs::remove_dir_all(&tmp); return None; }
    Some((tmp, wt))
}
pub fn worktree_remove(root: &Path, tmp: &Path, wt: &Path) { let _ = std::process::Command::new("git").args(["worktree", "remove", "--force", wt.to_str().unwrap_or("")]).current_dir(root).output(); let _ = std::fs::remove_dir_all(tmp); }
