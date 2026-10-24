//! グラフへの質問。CLI と MCP が同じ関数を使う。
//! 形:writes <table> / reads <table> / calls <fn> / callers <fn> / imports <file> / importers <file> / route <path> / trace <path> / node <name> / search <text> / env <KEY> / external <host>
use crate::model::*;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::path::Path;

pub struct Db { pub g: Graph, pub a: crate::analyze::Analysis }

pub fn load(out: &Path) -> Result<Db> {
    let data = if out.join("data").exists() { out.join("data") } else { out.to_path_buf() };
    let g: Graph = crate::store::read_json(&data.join("graph.json")).ok_or_else(|| anyhow!("graph.json が無い: {}", data.display()))?;
    let a: crate::analyze::Analysis = crate::store::read_json(&data.join("analysis.json")).unwrap_or_default();
    Ok(Db { g, a })
}

fn brief(n: &Node) -> Value { json!({"id": n.id, "kind": n.kind, "name": n.name, "path": n.path, "line": n.line, "layer": n.layer, "signature": n.signature, "doc": n.doc.lines().next().unwrap_or(""), "subkind": n.subkind, "hash": n.hash}) }

pub fn run(out: &Path, q: &str) -> Result<Value> { let db = load(out)?; Ok(ask(&db, q)) }

pub fn ask(db: &Db, q: &str) -> Value {
    let g = &db.g;
    let (cmd, arg) = q.trim().split_once(' ').map(|(a, b)| (a.to_lowercase(), b.trim().to_string())).unwrap_or((q.trim().to_lowercase(), String::new()));
    let find = |pred: &dyn Fn(&Node) -> bool| -> Vec<&Node> { g.nodes.iter().filter(|n| pred(n)).collect() };
    let by_id = |id: &str| g.nodes.iter().find(|n| n.id == id);
    let matches_fn = |n: &Node, s: &str| n.kind == NodeKind::Function && (n.name == s || n.name.ends_with(&format!(".{}", s)) || n.id == s || format!("{}#{}", n.path, n.name) == s);
    match cmd.as_str() {
        "writes" | "reads" => { let kind = if cmd == "writes" { EdgeKind::Writes } else { EdgeKind::Reads }; let t = format!("table:{}", arg); let fns: Vec<Value> = g.edges.iter().filter(|e| e.kind == kind && e.to == t).filter_map(|e| by_id(&e.from)).map(brief).collect(); json!({"table": arg, cmd: fns}) }
        "table" => { let t = find(&|n| n.kind == NodeKind::Table && n.name == arg); let (r, w) = db.a.table_access.get(&arg).cloned().unwrap_or_default(); json!({"table": t.first().map(|n| json!({"columns": n.attrs.get("columns"), "fks": n.attrs.get("fks"), "rls": n.attrs.get("rls"), "excerpt": n.excerpt})), "readers": r.iter().filter_map(|id| by_id(id)).map(brief).collect::<Vec<_>>(), "writers": w.iter().filter_map(|id| by_id(id)).map(brief).collect::<Vec<_>>()}) }
        "calls" => { let src: Vec<&Node> = find(&|n| matches_fn(n, &arg)); let out: Vec<Value> = src.iter().map(|s| json!({"from": brief(s), "calls": g.edges.iter().filter(|e| e.kind == EdgeKind::Calls && e.from == s.id).filter_map(|e| by_id(&e.to)).map(brief).collect::<Vec<_>>(), "unresolved": s.attrs.get("calls_unresolved")})).collect(); json!(out) }
        "callers" => { let tg: Vec<&Node> = find(&|n| matches_fn(n, &arg)); let out: Vec<Value> = tg.iter().map(|t| json!({"target": brief(t), "callers": g.edges.iter().filter(|e| e.kind == EdgeKind::Calls && e.to == t.id).filter_map(|e| by_id(&e.from)).map(brief).collect::<Vec<_>>(), "routes": g.edges.iter().filter(|e| e.kind == EdgeKind::Handles && e.to == t.id).filter_map(|e| by_id(&e.from)).map(brief).collect::<Vec<_>>()})).collect(); json!(out) }
        "imports" | "importers" => { let f = format!("file:{}", arg); let v: Vec<Value> = g.edges.iter().filter(|e| e.kind == EdgeKind::Imports && (if cmd == "imports" { e.from == f } else { e.to == f })).filter_map(|e| by_id(if cmd == "imports" { &e.to } else { &e.from })).map(brief).collect(); json!({cmd: v}) }
    "route" | "trace" => { let rs: Vec<&Node> = find(&|n| n.kind == NodeKind::Route && (n.name.contains(&arg) || n.path.contains(&arg))); let out: Vec<Value> = rs.iter().take(10).map(|r| { let t = db.a.traces.iter().find(|t| t.route == r.id); json!({"route": brief(r), "auth": r.attrs.get("auth"), "handler_excerpt": r.excerpt, "trace": t.map(|t| json!({"steps": t.steps, "reads": t.reads, "writes": t.writes, "externals": t.externals, "llm": t.llm}))}) }).collect(); json!(out) }
        "node" | "fn" | "type" => { let v: Vec<Value> = find(&|n| n.name == arg || n.id == arg || n.name.ends_with(&format!(".{}", arg))).iter().take(10).map(|n| json!({"node": brief(n), "excerpt": n.excerpt, "attrs": n.attrs, "edges_out": g.edges.iter().filter(|e| e.from == n.id && e.kind != EdgeKind::Contains).map(|e| json!([e.kind, e.to])).take(40).collect::<Vec<_>>(), "edges_in": g.edges.iter().filter(|e| e.to == n.id && e.kind != EdgeKind::Contains).map(|e| json!([e.kind, e.from])).take(40).collect::<Vec<_>>()})).collect(); json!(v) }
        "search" => { let a = arg.to_lowercase(); let v: Vec<Value> = find(&|n| n.name.to_lowercase().contains(&a) || n.doc.to_lowercase().contains(&a) || n.path.to_lowercase().contains(&a)).iter().take(50).map(|n| brief(n)).collect(); json!(v) }
        "env" => { let e = format!("env:{}", arg); let v: Vec<Value> = g.edges.iter().filter(|x| x.to == e).filter_map(|x| by_id(&x.from)).map(brief).collect(); json!({"env": arg, "used_by": v}) }
        "external" => { let v: Vec<Value> = g.edges.iter().filter(|x| x.kind == EdgeKind::Uses && x.to.starts_with("ext:") && x.to.contains(&arg)).filter_map(|x| by_id(&x.from)).map(brief).collect(); json!({"external": arg, "used_by": v}) }
        "routes" => json!(find(&|n| n.kind == NodeKind::Route).iter().map(|n| json!({"name": n.name, "kind": n.subkind, "auth": n.attrs.get("auth"), "path": n.path})).collect::<Vec<_>>()),
        "tables" => json!(find(&|n| n.kind == NodeKind::Table).iter().map(|n| json!({"name": n.name, "rls": n.attrs.get("rls"), "columns": n.attrs.get("columns").and_then(|c| c.as_array()).map(|a| a.len())})).collect::<Vec<_>>()),
        "metrics" => json!(db.a.metrics),
        "help" | "" => json!({"commands": ["writes <table>", "reads <table>", "table <name>", "calls <fn>", "callers <fn>", "imports <file>", "importers <file>", "route <path>", "node <name>", "search <text>", "env <KEY>", "external <host>", "routes", "tables", "metrics"]}),
        _ => json!({"error": format!("不明な質問: {}", q), "help": "help で一覧"}),
    }
}
