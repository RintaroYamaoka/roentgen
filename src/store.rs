//! 保存:graph.json / analysis.json / config.json(事実の正本)と graph.db(SQLite。query と MCP が引く)。
use crate::analyze::Analysis;
use crate::model::*;
use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

pub fn write_json<T: serde::Serialize>(path: &Path, v: &T) -> Result<()> { if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; } std::fs::write(path, serde_json::to_string(v)?)?; Ok(()) }
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> { std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()) }

pub fn write_db(path: &Path, g: &Graph, a: &Analysis) -> Result<()> {
    if path.exists() { std::fs::remove_file(path)?; }
    let mut c = Connection::open(path)?;
    c.execute_batch("pragma journal_mode=off; pragma synchronous=off;
        create table nodes(id text primary key, kind text, name text, path text, line int, end_line int, layer text, lang text, hash text, exported int, signature text, doc text, subkind text, attrs text, excerpt text);
        create table edges(src text, dst text, kind text, line int, unresolved int);
        create index e_src on edges(src); create index e_dst on edges(dst); create index n_kind on nodes(kind); create index n_name on nodes(name); create index n_path on nodes(path);
        create table traces(route text primary key, data text);
        create table meta(key text primary key, value text);")?;
    let tx = c.transaction()?;
    { let mut st = tx.prepare("insert into nodes values(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")?;
      for n in &g.nodes { st.execute(params![n.id, serde_json::to_string(&n.kind)?.trim_matches('"'), n.name, n.path, n.line, n.end_line, n.layer, n.lang, n.hash, n.exported as i32, n.signature, n.doc, n.subkind, serde_json::to_string(&n.attrs)?, n.excerpt])?; } }
    { let mut st = tx.prepare("insert into edges values(?,?,?,?,?)")?;
      for e in &g.edges { st.execute(params![e.from, e.to, serde_json::to_string(&e.kind)?.trim_matches('"'), e.line, e.unresolved as i32])?; } }
    { let mut st = tx.prepare("insert into traces values(?,?)")?; for t in &a.traces { st.execute(params![t.route, serde_json::to_string(t)?])?; } }
    tx.execute("insert into meta values('metrics', ?)", params![serde_json::to_string(&a.metrics)?])?;
    tx.commit()?;
    Ok(())
}
