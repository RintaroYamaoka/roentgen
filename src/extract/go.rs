//! Go(tree-sitter)。import・func / method・type(struct / interface)・呼び出し・doc コメント。
use super::ir::*;
use super::ts_util::*;
use tree_sitter::Node;

pub fn extract(path: &str, src: &str) -> FileIR {
    let mut ir = FileIR { path: path.into(), lang: "go".into(), lines: src.lines().count() as u32, ..Default::default() };
    let Some(tree) = parse(&tree_sitter_go::LANGUAGE.into(), src) else { ir.parse_error = true; return ir; };
    let root = tree.root_node();
    ir.parse_error = root.has_error();
    ir.env_keys = env_keys(src);
    for c in named_children(root) {
        match c.kind() {
            "import_declaration" => { walk(c, &mut |n| { if n.kind() == "import_spec" { let p = field(n, "path").map(|p| string_value(p, src)).unwrap_or_default(); let alias = field_text(n, "name", src); let ns = if alias.is_empty() || alias == "_" || alias == "." { p.rsplit('/').next().unwrap_or("").to_string() } else { alias }; ir.imports.push(Import { spec: p, names: vec![], line: line(n), namespace: Some(ns), default: None }); } true }); }
            "function_declaration" => ir.functions.push(func_of(c, src, None)),
            "method_declaration" => { let recv = field(c, "receiver").map(|r| text(r, src).trim_matches(|ch| ch == '(' || ch == ')').rsplit(' ').next().unwrap_or("").trim_start_matches('*').to_string()); ir.functions.push(func_of(c, src, recv.as_deref())); }
            "type_declaration" => { for s in named_children(c) { if s.kind() == "type_spec" { let name = field_text(s, "name", src); let ty = field(s, "type"); let kind = match ty.map(|t| t.kind()) { Some("struct_type") => "struct", Some("interface_type") => "interface", _ => "type" }; let mut t = TypeDef { name: name.clone(), line: line(c), end_line: end_line(c), exported: name.chars().next().map(|ch| ch.is_uppercase()).unwrap_or(false), kind: kind.into(), doc: doc_before(c, src), ..Default::default() }; if let Some(ty) = ty { let body = text(ty, src); t.refs = super::ts::type_refs(body); for l in body.lines() { let l = l.trim(); if kind == "struct" && l.split_whitespace().count() == 1 && l.chars().next().map(|ch| ch.is_uppercase()).unwrap_or(false) { t.extends.push(l.to_string()); } if kind == "interface" { if let Some(i) = l.find('(') { t.methods.push(l[..i].trim().to_string()); } else if l.split_whitespace().count() == 1 && l.chars().next().map(|ch| ch.is_uppercase()).unwrap_or(false) { t.extends.push(l.to_string()); } } } } t.text = excerpt(src, t.line, t.end_line, 40); ir.types.push(t); } } }
            "var_declaration" | "const_declaration" | "expression_statement" => { let mut f = Func::default(); collect_calls(c, src, &mut f); ir.top_calls.extend(f.calls); ir.top_strings.extend(f.strings); }
            "comment" => { if ir.doc.is_empty() && ir.functions.is_empty() && ir.types.is_empty() && line(c) <= 5 { ir.doc = clean_comment(text(c, src)); } }
            _ => {}
        }
    }
    ir
}

fn func_of(d: Node, src: &str, recv: Option<&str>) -> Func {
    let name = field_text(d, "name", src);
    let params = field_text(d, "parameters", src);
    let ret = field(d, "result").map(|r| format!(" {}", text(r, src))).unwrap_or_default();
    let mut f = Func { name: name.clone(), line: line(d), end_line: end_line(d), exported: name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false), kind: if recv.is_some() { "method" } else { "fn" }.into(), class: recv.map(|s| s.into()), doc: doc_before(d, src), signature: format!("{}{}", params, ret), ..Default::default() };
    if let Some(body) = field(d, "body") { collect_calls(body, src, &mut f); }
    f.body = excerpt(src, f.line, f.end_line, 60);
    f
}

fn collect_calls(n: Node, src: &str, f: &mut Func) {
    walk(n, &mut |c| {
        match c.kind() {
            "call_expression" => { if let Some(func) = field(c, "function") { match func.kind() { "identifier" => f.calls.push(Call { name: text(func, src).into(), receiver: None, line: line(c), arg: first_string_arg(field(c, "arguments"), src) }), "selector_expression" => { let name = field_text(func, "field", src); let op = field(func, "operand").map(|o| text(o, src).chars().take(120).collect()); f.calls.push(Call { name, receiver: op, line: line(c), arg: first_string_arg(field(c, "arguments"), src) }); } _ => {} } } true }
            "interpreted_string_literal" | "raw_string_literal" => { let v = string_value(c, src); if !v.is_empty() && v.len() < 400 { f.strings.push(v); } false }
            "func_literal" => true,
            _ => true,
        }
    });
}
