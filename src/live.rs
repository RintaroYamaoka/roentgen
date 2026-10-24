//! `--db` の実物(PostgreSQL)。psql CLI で information_schema を読む(接続文字列は出力に出さない)。
use crate::extract::sql::{Column, Fk, Policy, Table};
use anyhow::{anyhow, Result};
use std::collections::BTreeMap;

fn q(url: &str, sql: &str) -> Result<Vec<Vec<String>>> {
    let o = std::process::Command::new("psql").args([url, "-At", "-F", "\x1f", "-c", sql]).output().map_err(|_| anyhow!("psql が無い"))?;
    if !o.status.success() { return Err(anyhow!("psql が失敗した")); }
    Ok(String::from_utf8_lossy(&o.stdout).lines().filter(|l| !l.is_empty()).map(|l| l.split('\x1f').map(String::from).collect()).collect())
}

pub fn introspect(url: &str) -> Result<BTreeMap<String, Table>> {
    let mut t: BTreeMap<String, Table> = BTreeMap::new();
    for r in q(url, "select table_schema, table_name from information_schema.tables where table_schema not in ('pg_catalog','information_schema') and table_type='BASE TABLE' order by 1,2")? { let key = if r[0] == "public" { r[1].clone() } else { format!("{}.{}", r[0], r[1]) }; t.insert(key.clone(), Table { name: key, source: "live".into(), rows: Some(0), ..Default::default() }); }
    for r in q(url, "select table_schema, table_name, column_name, data_type, is_nullable from information_schema.columns where table_schema not in ('pg_catalog','information_schema') order by table_schema, table_name, ordinal_position")? { let key = if r[0] == "public" { r[1].clone() } else { format!("{}.{}", r[0], r[1]) }; if let Some(x) = t.get_mut(&key) { x.columns.push(Column { name: r[2].clone(), ty: r[3].clone(), nullable: r[4] == "YES" }); } }
    for r in q(url, "select tc.table_name, kcu.column_name, ccu.table_name from information_schema.table_constraints tc join information_schema.key_column_usage kcu on tc.constraint_name=kcu.constraint_name and tc.table_schema=kcu.table_schema join information_schema.constraint_column_usage ccu on ccu.constraint_name=tc.constraint_name where tc.constraint_type='FOREIGN KEY'")? { if let Some(x) = t.get_mut(&r[0]) { x.fks.push(Fk { column: r[1].clone(), ref_table: r[2].clone() }); } }
    for r in q(url, "select relname, relrowsecurity from pg_class c join pg_namespace n on n.oid=c.relnamespace where relkind='r' and nspname not in ('pg_catalog','information_schema')")? { if let Some(x) = t.get_mut(&r[0]) { x.rls = r[1] == "t"; } }
    for r in q(url, "select relname, n_live_tup from pg_stat_user_tables")? { if let Some(x) = t.get_mut(&r[0]) { x.rows = r[1].parse().ok(); } }
    for r in q(url, "select tablename, indexname from pg_indexes where schemaname not in ('pg_catalog','information_schema')")? { if let Some(x) = t.get_mut(&r[0]) { x.indexes.push(r[1].clone()); } }
    for r in q(url, "select tablename, policyname, cmd, roles::text from pg_policies")? { if let Some(x) = t.get_mut(&r[0]) { x.policies.push(Policy { name: r[1].clone(), cmd: r[2].clone(), roles: r[3].clone() }); } }
    Ok(t)
}

/// 実物を正にして、migration との差を文で返す。
pub fn merge(static_: &BTreeMap<String, Table>, live: BTreeMap<String, Table>) -> (BTreeMap<String, Table>, Vec<String>) {
    let mut drift = Vec::new();
    for k in live.keys() { if !static_.contains_key(k) { drift.push(format!("実物にだけある表 {}", k)); } }
    for k in static_.keys() { if !live.contains_key(k) { drift.push(format!("migration にだけある表 {}", k)); } }
    for (k, lt) in &live { if let Some(st) = static_.get(k) { let lc: std::collections::BTreeSet<&str> = lt.columns.iter().map(|c| c.name.as_str()).collect(); let sc: std::collections::BTreeSet<&str> = st.columns.iter().map(|c| c.name.as_str()).collect(); for c in lc.difference(&sc) { drift.push(format!("{}.{} は実物にだけある", k, c)); } for c in sc.difference(&lc) { drift.push(format!("{}.{} は migration にだけある", k, c)); } } }
    let mut out = live;
    for (k, v) in out.iter_mut() { if let Some(st) = static_.get(k) { v.first_migration = st.first_migration.clone(); v.last_migration = st.last_migration.clone(); } }
    (out, drift)
}
