//! Rust(tree-sitter)。use・mod・fn / impl のメソッド・struct / enum / trait / type・呼び出し・macro(sqlx::query! など)・attribute(#[get("/x")])・/// doc。
use super::ir::*;
use super::ts_util::*;
use tree_sitter::Node;

pub fn extract(path: &str, src: &str) -> FileIR {
    let mut ir = FileIR { path: path.into(), lang: "rs".into(), lines: src.lines().count() as u32, ..Default::default() };
    let Some(tree) = parse(&tree_sitter_rust::LANGUAGE.into(), src) else { ir.parse_error = true; return ir; };
    let root = tree.root_node();
    ir.parse_error = root.has_error();
    ir.env_keys = env_keys(src);
    for c in named_children(root) { if c.kind() == "line_comment" && text(c, src).starts_with("//!") { ir.doc.push_str(&clean_comment(text(c, src))); ir.doc.push(' '); } }
    ir.doc = ir.doc.trim().to_string();
    let mut pending_attrs: Vec<String> = Vec::new();
    for c in named_children(root) {
        match c.kind() {
            "attribute_item" => pending_attrs.push(text(c, src).to_string()),
            "use_declaration" => { for u in flatten_use(c, src) { ir.imports.push(Import { spec: u.0, names: vec![(u.1.clone(), u.1)], line: line(c), namespace: None, default: None }); } pending_attrs.clear(); }
            "mod_item" => { let name = field_text(c, "name", src); if field(c, "body").is_none() { ir.imports.push(Import { spec: format!("mod {}", name), names: vec![], line: line(c), namespace: Some(name), default: None }); } else if let Some(b) = field(c, "body") { for cc in named_children(b) { item(cc, src, &mut ir, None, &mut pending_attrs); } } pending_attrs.clear(); }
            _ => item(c, src, &mut ir, None, &mut pending_attrs),
        }
    }
    ir
}

fn flatten_use(n: Node, src: &str) -> Vec<(String, String)> {
    let t = text(n, src).trim_start_matches("pub ").trim_start_matches("pub(crate) ").trim_start_matches("use ").trim_end_matches(';').trim();
    let mut out = Vec::new();
    expand(t, &mut out);
    out.into_iter().map(|p| { let name = p.rsplit("::").next().unwrap_or("").split(" as ").last().unwrap_or("").trim().to_string(); (p.split(" as ").next().unwrap_or("").trim().to_string(), name) }).collect()
}

fn expand(t: &str, out: &mut Vec<String>) {
    if let Some(i) = t.find('{') {
        let prefix = &t[..i];
        let inner = &t[i + 1..t.rfind('}').unwrap_or(t.len())];
        let mut depth = 0; let mut cur = String::new();
        for ch in inner.chars() { match ch { '{' => { depth += 1; cur.push(ch) } '}' => { depth -= 1; cur.push(ch) } ',' if depth == 0 => { expand(&format!("{}{}", prefix, cur.trim()), out); cur.clear(); } _ => cur.push(ch) } }
        if !cur.trim().is_empty() { expand(&format!("{}{}", prefix, cur.trim()), out); }
    } else if !t.is_empty() { out.push(t.trim().trim_start_matches("::").to_string()); }
}

fn is_pub(n: Node, src: &str) -> bool { children(n).iter().any(|c| c.kind() == "visibility_modifier") || text(n, src).starts_with("pub") }

fn item(c: Node, src: &str, ir: &mut FileIR, impl_of: Option<&str>, attrs: &mut Vec<String>) {
    match c.kind() {
        "attribute_item" => { attrs.push(text(c, src).to_string()); return; }
        "function_item" | "function_signature_item" => { let mut f = func_of(c, src, impl_of); f.decorators = attrs.clone(); ir.functions.push(f); }
        "impl_item" => {
            let ty = field(c, "type").map(|t| text(t, src).split('<').next().unwrap_or("").to_string()).unwrap_or_default();
            let tr = field(c, "trait").map(|t| text(t, src).split('<').next().unwrap_or("").rsplit("::").next().unwrap_or("").to_string());
            if let Some(trait_name) = &tr { if let Some(t) = ir.types.iter_mut().find(|t| t.name == ty) { t.implements.push(trait_name.clone()); } else { ir.types.push(TypeDef { name: ty.clone(), line: line(c), end_line: end_line(c), exported: true, kind: "struct".into(), implements: vec![trait_name.clone()], ..Default::default() }); } }
            if let Some(body) = field(c, "body") { let mut a = Vec::new(); for m in named_children(body) { if m.kind() == "function_item" { let mut f = func_of(m, src, Some(&ty)); f.decorators = a.clone(); a.clear(); if let Some(t) = ir.types.iter_mut().find(|t| t.name == ty) { t.methods.push(f.name.clone()); } ir.functions.push(f); } else if m.kind() == "attribute_item" { a.push(text(m, src).to_string()); } } }
        }
        "struct_item" | "enum_item" | "trait_item" | "type_item" | "union_item" => {
            let kind = match c.kind() { "struct_item" | "union_item" => "struct", "enum_item" => "enum", "trait_item" => "trait", _ => "type" };
            let name = field_text(c, "name", src);
            let mut t = TypeDef { name: name.clone(), line: line(c), end_line: end_line(c), exported: is_pub(c, src), kind: kind.into(), doc: doc_before(c, src), ..Default::default() };
            if let Some(body) = field(c, "body") { t.refs = super::ts::type_refs(text(body, src)); if kind == "trait" { for m in named_children(body) { if m.kind() == "function_signature_item" || m.kind() == "function_item" { t.methods.push(field_text(m, "name", src)); } } } }
            if let Some(b) = field(c, "bounds") { t.extends = named_children(b).iter().map(|x| text(*x, src).split('<').next().unwrap_or("").rsplit("::").next().unwrap_or("").to_string()).collect(); }
            t.text = excerpt(src, t.line, t.end_line, 40);
            if let Some(existing) = ir.types.iter_mut().find(|x| x.name == name && x.text.is_empty()) { let impls = existing.implements.clone(); *existing = t; existing.implements = impls; } else { ir.types.push(t); }
        }
        "macro_invocation" | "expression_statement" | "let_declaration" | "static_item" | "const_item" => { let mut f = Func::default(); collect_calls(c, src, &mut f); ir.top_calls.extend(f.calls); ir.top_strings.extend(f.strings); }
        _ => {}
    }
    attrs.clear();
}

fn func_of(d: Node, src: &str, impl_of: Option<&str>) -> Func {
    let name = field_text(d, "name", src);
    let params = field_text(d, "parameters", src);
    let ret = field(d, "return_type").map(|r| format!(" -> {}", text(r, src))).unwrap_or_default();
    let is_async = text(d, src).contains("async fn");
    let mut f = Func { name, line: line(d), end_line: end_line(d), exported: is_pub(d, src) || impl_of.is_some(), kind: if impl_of.is_some() { "method" } else { "fn" }.into(), class: impl_of.map(|s| s.into()), is_async, doc: doc_before(d, src), ..Default::default() };
    f.signature = format!("{}{}{}", if is_async { "async " } else { "" }, params, ret);
    if let Some(body) = field(d, "body") { collect_calls(body, src, &mut f); }
    f.body = excerpt(src, f.line, f.end_line, 60);
    f
}

fn collect_calls(n: Node, src: &str, f: &mut Func) {
    walk(n, &mut |c| {
        match c.kind() {
            "call_expression" => { if let Some(func) = field(c, "function") { match func.kind() { "identifier" => f.calls.push(Call { name: text(func, src).into(), receiver: None, line: line(c), arg: first_string_arg(field(c, "arguments"), src) }), "scoped_identifier" => { let name = field_text(func, "name", src); let path = field(func, "path").map(|p| text(p, src).to_string()); f.calls.push(Call { name, receiver: path, line: line(c), arg: first_string_arg(field(c, "arguments"), src) }); } "field_expression" => { let name = field_text(func, "field", src); let v = field(func, "value").map(|p| text(p, src).chars().take(120).collect()); f.calls.push(Call { name, receiver: v, line: line(c), arg: first_string_arg(field(c, "arguments"), src) }); } "generic_function" => { if let Some(inner) = field(func, "function") { f.calls.push(Call { name: text(inner, src).rsplit("::").next().unwrap_or("").into(), receiver: None, line: line(c), arg: first_string_arg(field(c, "arguments"), src) }); } } _ => {} } } true }
            "macro_invocation" => { let m = field_text(c, "macro", src); let arg = { let t = text(c, src); let rx = once_cell::sync::Lazy::<regex::Regex>::new(|| regex::Regex::new(r##"^[^(]*\(\s*r?#*"((?s).*?)"#*\s*[,)]"##).unwrap()); rx.captures(t).map(|x| x[1].chars().take(400).collect::<String>()) }; f.calls.push(Call { name: format!("{}!", m.rsplit("::").next().unwrap_or(&m)), receiver: if m.contains("::") { Some(m.rsplit_once("::").map(|x| x.0.to_string()).unwrap_or_default()) } else { None }, line: line(c), arg }); true }
            "string_literal" | "raw_string_literal" => { let v = string_value(c, src); if !v.is_empty() && v.len() < 400 { f.strings.push(v); } false }
            "function_item" | "closure_expression" => c.kind() == "closure_expression",
            _ => true,
        }
    });
}
