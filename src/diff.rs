//! グラフ同士の差分:ノードの追加 / 削除 / 変化(hash)と、意味のある辺の増減(route の認可・表の読み書き・禁止辺・cycles)。
use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Diff { pub base: serde_json::Value, pub changed: Vec<String>, pub sections: BTreeMap<String, Section> }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Section { pub added: Vec<String>, pub removed: Vec<String>, pub changed: Vec<(String, String)> }

pub fn compare(prev: &Path, cur: &Path) -> Diff {
    let pg: Graph = crate::store::read_json(&prev.join("graph.json")).unwrap_or_default();
    let cg: Graph = crate::store::read_json(&cur.join("graph.json")).unwrap_or_default();
    let pa: crate::analyze::Analysis = crate::store::read_json(&prev.join("analysis.json")).unwrap_or_default();
    let ca: crate::analyze::Analysis = crate::store::read_json(&cur.join("analysis.json")).unwrap_or_default();
    let mut d = Diff { base: crate::store::read_json::<serde_json::Value>(&prev.join("meta.json")).unwrap_or_default(), ..Default::default() };
    let label = |n: &Node| match n.kind { NodeKind::Function | NodeKind::Type | NodeKind::Route => format!("{} ({})", n.name, n.path), _ => n.name.clone() };
    for (key, kind) in [("tables", NodeKind::Table), ("routes", NodeKind::Route), ("functions", NodeKind::Function), ("types", NodeKind::Type), ("externals", NodeKind::External), ("packages", NodeKind::Package), ("env", NodeKind::Env), ("jobs", NodeKind::Job), ("files", NodeKind::File)] {
        let p: BTreeMap<&str, &Node> = pg.nodes.iter().filter(|n| n.kind == kind).map(|n| (n.id.as_str(), n)).collect();
        let c: BTreeMap<&str, &Node> = cg.nodes.iter().filter(|n| n.kind == kind).map(|n| (n.id.as_str(), n)).collect();
        let mut s = Section::default();
        for (id, n) in &c { if !p.contains_key(id) { s.added.push(label(n)); } }
        for (id, n) in &p { if !c.contains_key(id) { s.removed.push(label(n)); } }
        for (id, n) in &c { if let Some(o) = p.get(id) {
            if kind == NodeKind::Route { let (a, b) = (o.attrs.get("auth"), n.attrs.get("auth")); if a != b { s.changed.push((label(n), format!("auth {} → {}", a.and_then(|x| x.as_str()).unwrap_or("?"), b.and_then(|x| x.as_str()).unwrap_or("?")))); } }
            else if kind == NodeKind::Table { let cols = |x: &Node| x.attrs.get("columns").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|c| c.get("name").and_then(|s| s.as_str()).map(String::from)).collect::<BTreeSet<_>>()).unwrap_or_default(); let (a, b) = (cols(o), cols(n)); let add: Vec<_> = b.difference(&a).cloned().collect(); let rem: Vec<_> = a.difference(&b).cloned().collect(); let rls = (o.attrs.get("rls"), n.attrs.get("rls")); let mut parts = vec![]; if !add.is_empty() { parts.push(format!("列 追加 {}", add.join(", "))); } if !rem.is_empty() { parts.push(format!("列 削除 {}", rem.join(", "))); } if rls.0 != rls.1 { parts.push(format!("RLS {:?} → {:?}", rls.0.and_then(|x| x.as_bool()), rls.1.and_then(|x| x.as_bool()))); } if !parts.is_empty() { s.changed.push((label(n), parts.join("; "))); } }
            else if matches!(kind, NodeKind::Function | NodeKind::Type) && o.hash != n.hash && !n.hash.is_empty() { if kind == NodeKind::Function && s.changed.len() < 300 { s.changed.push((label(n), if o.signature != n.signature { format!("signature {} → {}", o.signature.chars().take(60).collect::<String>(), n.signature.chars().take(60).collect::<String>()) } else { "本文が変わった".into() })); } else if kind == NodeKind::Type && s.changed.len() < 300 { s.changed.push((label(n), "定義が変わった".into())); } }
        } }
        s.added.truncate(300); s.removed.truncate(300);
        d.sections.insert(key.into(), s);
    }
    // 意味のある辺:表の読み書きが増えた / 減った関数
    let acc = |g: &Graph| -> BTreeSet<String> { g.edges.iter().filter(|e| matches!(e.kind, EdgeKind::Reads | EdgeKind::Writes)).map(|e| format!("{} {} {}", e.from.trim_start_matches("fn:"), if e.kind == EdgeKind::Writes { "writes" } else { "reads" }, e.to.trim_start_matches("table:"))).collect() };
    let (pa_, ca_) = (acc(&pg), acc(&cg));
    d.sections.insert("table_access".into(), Section { added: ca_.difference(&pa_).cloned().take(200).collect(), removed: pa_.difference(&ca_).cloned().take(200).collect(), changed: vec![] });
    // 呼び出しの辺(入口から見た trace の変化)
    let tr = |a: &crate::analyze::Analysis| -> BTreeMap<String, (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>)> { a.traces.iter().map(|t| (t.route.clone(), (t.steps.iter().map(|s| s.id.clone()).collect(), t.reads.iter().chain(t.writes.iter()).cloned().collect(), t.externals.iter().cloned().collect()))).collect() };
    let (pt, ct) = (tr(&pa), tr(&ca));
    let mut s = Section::default();
    for (r, (fs, ts, es)) in &ct { if let Some((pfs, pts, pes)) = pt.get(r) { let mut parts = vec![]; let fa: Vec<_> = fs.difference(pfs).map(|x| x.rsplit('#').next().unwrap_or(x).to_string()).collect(); let fr: Vec<_> = pfs.difference(fs).map(|x| x.rsplit('#').next().unwrap_or(x).to_string()).collect(); if !fa.is_empty() { parts.push(format!("呼ぶようになった {}", fa.iter().take(6).cloned().collect::<Vec<_>>().join(", "))); } if !fr.is_empty() { parts.push(format!("呼ばなくなった {}", fr.iter().take(6).cloned().collect::<Vec<_>>().join(", "))); } let ta: Vec<_> = ts.difference(pts).cloned().collect(); let trm: Vec<_> = pts.difference(ts).cloned().collect(); if !ta.is_empty() { parts.push(format!("表 追加 {}", ta.join(", "))); } if !trm.is_empty() { parts.push(format!("表 削除 {}", trm.join(", "))); } let ea: Vec<_> = es.difference(pes).cloned().collect(); if !ea.is_empty() { parts.push(format!("外部 追加 {}", ea.join(", "))); } if !parts.is_empty() { s.changed.push((r.rsplit('#').next().unwrap_or(r).to_string(), parts.join("; "))); } } }
    s.changed.truncate(200);
    d.sections.insert("traces".into(), s);
    // 規則と循環
    let mut s = Section::default();
    for r in &ca.rules { if let Some(o) = pa.rules.iter().find(|x| x.from == r.from && x.to == r.to) { if o.count != r.count { s.changed.push((format!("{} → {}", r.from, r.to), format!("{} → {}", o.count, r.count))); } } }
    let pc: BTreeSet<String> = pa.cycles.iter().map(|c| format!("{} ⇄ {}", c.a, c.b)).collect(); let cc: BTreeSet<String> = ca.cycles.iter().map(|c| format!("{} ⇄ {}", c.a, c.b)).collect();
    s.added = cc.difference(&pc).cloned().collect(); s.removed = pc.difference(&cc).cloned().collect();
    d.sections.insert("architecture".into(), s);
    d.changed = d.sections.iter().filter(|(_, s)| !(s.added.is_empty() && s.removed.is_empty() && s.changed.is_empty())).map(|(k, _)| k.clone()).collect();
    d
}
