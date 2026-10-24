//! notes = 解説キャッシュ。repo の .roentgen/notes/<anchor>.md にノードの説明を置き、frontmatter の hash で鮮度を見る。
//! Claude が書き、render が読む。hash が変わったノードだけ書き直せばよい。
use crate::model::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Note { pub id: String, pub hash: String, pub body: String, pub stale: bool }

pub fn dir(repo: &Path) -> std::path::PathBuf { repo.join(".roentgen").join("notes") }

/// repo の notes を読む(id → Note)。hash が今のノードと違えば stale。
pub fn load(repo: &Path, g: &Graph) -> BTreeMap<String, Note> {
    let mut out = BTreeMap::new();
    let d = dir(repo);
    let Ok(rd) = std::fs::read_dir(&d) else { return out };
    let hashes: BTreeMap<&str, &str> = g.nodes.iter().map(|n| (n.id.as_str(), n.hash.as_str())).collect();
    for e in rd.flatten() {
        let Ok(txt) = std::fs::read_to_string(e.path()) else { continue };
        let (fm, body) = split_frontmatter(&txt);
        let id = fm.get("id").cloned().unwrap_or_default();
        let hash = fm.get("hash").cloned().unwrap_or_default();
        if id.is_empty() { continue; }
        let stale = hashes.get(id.as_str()).map(|h| *h != hash).unwrap_or(true);
        out.insert(id.clone(), Note { id, hash, body: body.trim().to_string(), stale });
    }
    out
}

fn split_frontmatter(txt: &str) -> (BTreeMap<String, String>, String) {
    let mut fm = BTreeMap::new();
    if let Some(rest) = txt.strip_prefix("---\n") { if let Some(i) = rest.find("\n---") { for l in rest[..i].lines() { if let Some((k, v)) = l.split_once(':') { fm.insert(k.trim().into(), v.trim().into()); } } return (fm, rest[i + 4..].to_string()); } }
    (fm, txt.to_string())
}

pub fn write(repo: &Path, n: &Node, body: &str) -> Result<()> {
    let d = dir(repo); std::fs::create_dir_all(&d)?;
    let p = d.join(format!("{}.md", anchor(&n.id)));
    std::fs::write(p, format!("---\nid: {}\nhash: {}\nname: {}\npath: {}\n---\n{}\n", n.id, n.hash, n.name, n.path, body.trim()))?;
    Ok(())
}

/// status:未記入と古くなったノードを、重要度順(入口のハンドラ → 被呼び出しが多い関数 → 表 → 型)で出す。
pub fn cli(out: &Path, repo: Option<&Path>, action: &str) -> Result<()> {
    let db = crate::query::load(out)?;
    let repo = repo.map(|p| p.to_path_buf()).or_else(|| crate::store::read_json::<serde_json::Value>(&out.join("data/meta.json")).and_then(|_| None)).unwrap_or_else(|| std::env::current_dir().unwrap());
    let notes = load(&repo, &db.g);
    let mut in_deg: BTreeMap<&str, u32> = BTreeMap::new();
    for e in &db.g.edges { if matches!(e.kind, EdgeKind::Calls | EdgeKind::Handles) { *in_deg.entry(e.to.as_str()).or_insert(0) += 1; } }
    let mut items: Vec<(i64, &Node, &str)> = db.g.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Table | NodeKind::Type | NodeKind::Route) && !crate::repo::is_test(&n.path)).map(|n| {
        let st = match notes.get(&n.id) { None => "missing", Some(x) if x.stale => "stale", _ => "ok" };
        let score = match n.kind { NodeKind::Route => 1000 + if n.subkind == "cli" { -400 } else { 0 }, NodeKind::Table => 800, NodeKind::Function => 100 + (*in_deg.get(n.id.as_str()).unwrap_or(&0) as i64 * 5).min(500), NodeKind::Type => 50 + (*in_deg.get(n.id.as_str()).unwrap_or(&0) as i64).min(100), _ => 0 } + if n.exported { 20 } else { 0 };
        (score, n, st)
    }).collect();
    items.sort_by(|a, b| b.0.cmp(&a.0));
    match action {
        "status" => {
            let missing = items.iter().filter(|x| x.2 == "missing").count(); let stale = items.iter().filter(|x| x.2 == "stale").count(); let ok = items.iter().filter(|x| x.2 == "ok").count();
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({"notes_dir": dir(&repo), "ok": ok, "stale": stale, "missing": missing,
                "todo": items.iter().filter(|x| x.2 != "ok").take(60).map(|(s, n, st)| serde_json::json!({"id": n.id, "kind": n.kind, "name": n.name, "path": n.path, "line": n.line, "status": st, "score": s, "hash": n.hash, "doc": n.doc.lines().next().unwrap_or("")})).collect::<Vec<_>>()}))?);
        }
        _ => println!("action: status"),
    }
    Ok(())
}
