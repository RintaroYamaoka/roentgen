//! SQL migration(sqlparser)→ 表・列・FK・RLS・policy・index。Prisma / Drizzle / Diesel は簡易に読む。
use serde::{Deserialize, Serialize};
use sqlparser::ast::*;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Column { pub name: String, pub ty: String, pub nullable: bool }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Fk { pub column: String, pub ref_table: String }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Policy { pub name: String, pub cmd: String, pub roles: String }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Table {
    pub name: String, pub columns: Vec<Column>, pub fks: Vec<Fk>, pub rls: bool, pub policies: Vec<Policy>, pub indexes: Vec<String>,
    pub first_migration: String, pub last_migration: String, pub source: String, pub rows: Option<i64>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Migration { pub file: String, pub date: String, pub tables: Vec<String>, pub parse_errors: u32 }

fn norm(n: &ObjectName) -> String {
    let parts: Vec<String> = n.0.iter().map(|p| p.to_string().trim_matches('"').to_string()).collect();
    let s = parts.join(".");
    s.strip_prefix("public.").unwrap_or(&s).to_string()
}

fn mig_date(file: &str) -> String {
    static RX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"(20\d{2})[-_]?(\d{2})[-_]?(\d{2})").unwrap());
    let base = file.rsplit('/').next().unwrap_or(file);
    RX.captures(base).map(|c| format!("{}-{}-{}", &c[1], &c[2], &c[3])).unwrap_or_default()
}

/// ; で分けて 1 文ずつ parse(1 文が壊れても残りを読む)。
fn statements(sql: &str) -> (Vec<Statement>, u32) {
    let dialect = PostgreSqlDialect {};
    match Parser::parse_sql(&dialect, sql) { Ok(v) => (v, 0), Err(_) => {
        let mut out = Vec::new(); let mut errs = 0;
        for piece in split_sql(sql) { match Parser::parse_sql(&dialect, &piece) { Ok(mut v) => out.append(&mut v), Err(_) => errs += 1 } }
        (out, errs)
    } }
}

fn split_sql(sql: &str) -> Vec<String> {
    let mut out = Vec::new(); let mut cur = String::new(); let mut in_dollar = false; let mut in_str = false;
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '$' && i + 1 < chars.len() && chars[i + 1] == '$' { in_dollar = !in_dollar; cur.push_str("$$"); i += 2; continue; }
        if c == '\'' && !in_dollar { in_str = !in_str; }
        if c == ';' && !in_dollar && !in_str { out.push(cur.clone()); cur.clear(); } else { cur.push(c); }
        i += 1;
    }
    if !cur.trim().is_empty() { out.push(cur); }
    out.into_iter().filter(|s| !s.trim().is_empty()).collect()
}

pub fn from_sql_files(files: &[(String, String)]) -> (BTreeMap<String, Table>, Vec<Migration>) {
    let mut tables: BTreeMap<String, Table> = BTreeMap::new();
    let mut migs = Vec::new();
    for (file, sql) in files {
        let date = mig_date(file);
        let (stmts, errs) = statements(sql);
        let mut touched: Vec<String> = Vec::new();
        for st in stmts {
            match st {
                Statement::CreateTable(ct) => {
                    let name = norm(&ct.name);
                    let t = tables.entry(name.clone()).or_insert_with(|| Table { name: name.clone(), first_migration: date.clone(), source: file.clone(), ..Default::default() });
                    t.last_migration = date.clone();
                    for c in &ct.columns {
                        let nullable = !c.options.iter().any(|o| matches!(o.option, ColumnOption::NotNull) || matches!(o.option, ColumnOption::Unique { is_primary: true, .. }));
                        if !t.columns.iter().any(|x| x.name == c.name.value) { t.columns.push(Column { name: c.name.value.clone(), ty: c.data_type.to_string().chars().take(40).collect(), nullable }); }
                        for o in &c.options { if let ColumnOption::ForeignKey { foreign_table, .. } = &o.option { t.fks.push(Fk { column: c.name.value.clone(), ref_table: norm(foreign_table) }); } }
                    }
                    for tc in &ct.constraints { if let TableConstraint::ForeignKey { columns, foreign_table, .. } = tc { for col in columns { t.fks.push(Fk { column: col.to_string(), ref_table: norm(foreign_table) }); } } }
                    touched.push(name);
                }
                Statement::AlterTable { name, operations, .. } => {
                    let tname = norm(&name);
                    for op in operations {
                        match op {
                            AlterTableOperation::AddColumn { column_def, .. } => { let t = tables.entry(tname.clone()).or_insert_with(|| Table { name: tname.clone(), first_migration: date.clone(), source: file.clone(), ..Default::default() }); if !t.columns.iter().any(|x| x.name == column_def.name.value) { t.columns.push(Column { name: column_def.name.value.clone(), ty: column_def.data_type.to_string().chars().take(40).collect(), nullable: !column_def.options.iter().any(|o| matches!(o.option, ColumnOption::NotNull)) }); } for o in &column_def.options { if let ColumnOption::ForeignKey { foreign_table, .. } = &o.option { t.fks.push(Fk { column: column_def.name.value.clone(), ref_table: norm(foreign_table) }); } } t.last_migration = date.clone(); }
                            AlterTableOperation::DropColumn { column_names, .. } => { if let Some(t) = tables.get_mut(&tname) { for cn in column_names { t.columns.retain(|c| c.name != cn.value); } } }
                            AlterTableOperation::AddConstraint { constraint, .. } => { if let TableConstraint::ForeignKey { columns, foreign_table, .. } = constraint { if let Some(t) = tables.get_mut(&tname) { for col in columns { t.fks.push(Fk { column: col.to_string(), ref_table: norm(&foreign_table) }); } } } }
                            AlterTableOperation::EnableRowLevelSecurity => { if let Some(t) = tables.get_mut(&tname) { t.rls = true; } }
                            AlterTableOperation::DisableRowLevelSecurity => { if let Some(t) = tables.get_mut(&tname) { t.rls = false; } }
                            AlterTableOperation::RenameTable { table_name } => { if let Some(mut t) = tables.remove(&tname) { let new = norm(&table_name); t.name = new.clone(); tables.insert(new, t); } }
                            AlterTableOperation::RenameColumn { old_column_name, new_column_name } => { if let Some(t) = tables.get_mut(&tname) { for c in &mut t.columns { if c.name == old_column_name.value { c.name = new_column_name.value.clone(); } } } }
                            _ => {}
                        }
                    }
                    touched.push(tname);
                }
                Statement::Drop { object_type: ObjectType::Table, names, .. } => { for n in names { tables.remove(&norm(&n)); } }
                Statement::CreatePolicy { name, table_name, command, to, .. } => { let tn = norm(&table_name); if let Some(t) = tables.get_mut(&tn) { t.policies.push(Policy { name: name.to_string().chars().take(60).collect(), cmd: command.map(|c| format!("{:?}", c).to_uppercase()).unwrap_or("ALL".into()), roles: to.map(|r| r.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")).unwrap_or_default() }); touched.push(tn); } }
                Statement::CreateIndex(ci) => { let tn = norm(&ci.table_name); if let Some(t) = tables.get_mut(&tn) { t.indexes.push(ci.name.map(|n| n.to_string()).unwrap_or_default()); } }
                _ => {}
            }
        }
        // sqlparser が読めない方言(enable row level security が未対応の版など)は正規表現で補う
        regex_supplement(sql, &mut tables, &mut touched, &date, file);
        touched.sort(); touched.dedup();
        migs.push(Migration { file: file.clone(), date, tables: touched, parse_errors: errs });
    }
    (tables, migs)
}

fn regex_supplement(sql: &str, tables: &mut BTreeMap<String, Table>, touched: &mut Vec<String>, date: &str, file: &str) {
    static RLS: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r#"(?i)alter\s+table\s+(?:if\s+exists\s+)?(?:only\s+)?(?:"?public"?\.)?"?(\w+)"?\s+(enable|disable)\s+row\s+level\s+security"#).unwrap());
    static POL: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r#"(?is)create\s+policy\s+(?:"([^"]+)"|(\S+))\s+on\s+(?:"?public"?\.)?"?(\w+)"?(?:\s+as\s+\w+)?(?:\s+for\s+(\w+))?(?:\s+to\s+([\w, ]+?))?\s*(?:using|with|;)"#).unwrap());
    static CREATE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r#"(?is)create\s+table\s+(?:if\s+not\s+exists\s+)?(?:"?public"?\.)?"?(\w+)"?\s*\((.*?)\)\s*(?:;|$|with|partition|inherits)"#).unwrap());
    for c in RLS.captures_iter(sql) { if let Some(t) = tables.get_mut(&c[1]) { t.rls = c[2].eq_ignore_ascii_case("enable"); touched.push(c[1].to_string()); } }
    for c in POL.captures_iter(sql) {
        let tn = c[3].to_string();
        if let Some(t) = tables.get_mut(&tn) { let name = c.get(1).or(c.get(2)).map(|m| m.as_str()).unwrap_or(""); if !t.policies.iter().any(|p| p.name == name) { t.policies.push(Policy { name: name.chars().take(60).collect(), cmd: c.get(4).map(|m| m.as_str().to_uppercase()).unwrap_or("ALL".into()), roles: c.get(5).map(|m| m.as_str().trim().to_string()).unwrap_or_default() }); } touched.push(tn); }
    }
    // parse に失敗した CREATE TABLE を列だけ拾う
    for c in CREATE.captures_iter(sql) {
        let name = c[1].to_string();
        if tables.contains_key(&name) { continue; }
        let mut t = Table { name: name.clone(), first_migration: date.into(), last_migration: date.into(), source: file.into(), ..Default::default() };
        for col in split_cols(&c[2]) {
            let col = col.trim();
            let lower = col.to_lowercase();
            if lower.starts_with("primary key") || lower.starts_with("unique") || lower.starts_with("constraint") || lower.starts_with("check") || lower.starts_with("foreign key") { continue; }
            let mut it = col.split_whitespace();
            if let (Some(n), Some(ty)) = (it.next(), it.next()) { t.columns.push(Column { name: n.trim_matches('"').into(), ty: ty.chars().take(40).collect(), nullable: !lower.contains("not null") && !lower.contains("primary key") }); if let Some(i) = lower.find("references ") { let r = col[i + 11..].split(|ch: char| ch == '(' || ch.is_whitespace()).next().unwrap_or("").trim_matches('"'); t.fks.push(Fk { column: n.trim_matches('"').into(), ref_table: r.strip_prefix("public.").unwrap_or(r).into() }); } }
        }
        touched.push(name.clone());
        tables.insert(name, t);
    }
}

fn split_cols(body: &str) -> Vec<String> {
    let mut out = Vec::new(); let mut depth = 0; let mut cur = String::new();
    for ch in body.chars() { match ch { '(' => { depth += 1; cur.push(ch) } ')' => { depth -= 1; cur.push(ch) } ',' if depth == 0 => { out.push(cur.clone()); cur.clear() } _ => cur.push(ch) } }
    if !cur.trim().is_empty() { out.push(cur); }
    out
}

pub fn from_prisma(file: &str, src: &str) -> BTreeMap<String, Table> {
    static MODEL: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"(?s)model\s+(\w+)\s*\{(.*?)\n\}").unwrap());
    static MAP: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r#"@@map\("(\w+)"\)"#).unwrap());
    static REL: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"@relation\([^)]*fields:\s*\[(\w+)\]").unwrap());
    let mut out = BTreeMap::new();
    for m in MODEL.captures_iter(src) {
        let name = MAP.captures(&m[2]).map(|c| c[1].to_string()).unwrap_or(m[1].to_string());
        let mut t = Table { name: name.clone(), source: file.into(), rls: true, ..Default::default() };
        for line in m[2].lines() {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() < 2 || p[0].starts_with("@") { continue; }
            if let Some(r) = REL.captures(line) { t.fks.push(Fk { column: r[1].into(), ref_table: p[1].trim_end_matches(|c| c == '?' || c == '[' || c == ']').into() }); continue; }
            let ty = p[1].trim_end_matches('?').trim_end_matches("[]");
            if ty.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && !matches!(ty, "String" | "Int" | "BigInt" | "Float" | "Decimal" | "Boolean" | "DateTime" | "Json" | "Bytes") { continue; }
            t.columns.push(Column { name: p[0].into(), ty: p[1].into(), nullable: p[1].ends_with('?') });
        }
        out.insert(name, t);
    }
    out
}

pub fn from_drizzle(file: &str, src: &str) -> BTreeMap<String, Table> {
    static T: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r#"(?s)(?:pg|mysql|sqlite)Table\(\s*['"](\w+)['"]\s*,\s*\{(.*?)\n\s*\}"#).unwrap());
    static C: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"(\w+)\s*:\s*(\w+)\(([^)]*)\)([^,\n]*)").unwrap());
    static R: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"references\(\(\)\s*=>\s*(\w+)\.").unwrap());
    let mut out = BTreeMap::new();
    for m in T.captures_iter(src) {
        let mut t = Table { name: m[1].into(), source: file.into(), rls: true, ..Default::default() };
        for c in C.captures_iter(&m[2]) { t.columns.push(Column { name: c[1].into(), ty: c[2].into(), nullable: !c[4].contains(".notNull()") && !c[4].contains(".primaryKey()") }); if let Some(r) = R.captures(&c[4]) { t.fks.push(Fk { column: c[1].into(), ref_table: r[1].into() }); } }
        out.insert(m[1].to_string(), t);
    }
    out
}

pub fn from_diesel(file: &str, src: &str) -> BTreeMap<String, Table> {
    static T: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"(?s)table!\s*\{\s*(\w+)\s*\([^)]*\)\s*\{(.*?)\}\s*\}").unwrap());
    static C: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"(\w+)\s*->\s*(Nullable<)?(\w+)").unwrap());
    static J: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"joinable!\(\s*(\w+)\s*->\s*(\w+)\s*\((\w+)\)").unwrap());
    let mut out: BTreeMap<String, Table> = BTreeMap::new();
    for m in T.captures_iter(src) { let mut t = Table { name: m[1].into(), source: file.into(), rls: true, ..Default::default() }; for c in C.captures_iter(&m[2]) { t.columns.push(Column { name: c[1].into(), ty: c[3].into(), nullable: c.get(2).is_some() }); } out.insert(m[1].to_string(), t); }
    for j in J.captures_iter(src) { if let Some(t) = out.get_mut(&j[1]) { t.fks.push(Fk { column: j[3].into(), ref_table: j[2].into() }); } }
    out
}
