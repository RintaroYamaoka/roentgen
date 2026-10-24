//! Python(tree-sitter)。import・関数・クラス(bases / Protocol / dataclass)・呼び出し・docstring・decorator。
use super::ir::*;
use super::ts_util::*;
use tree_sitter::Node;

pub fn extract(path: &str, src: &str) -> FileIR {
    let mut ir = FileIR { path: path.into(), lang: "py".into(), lines: src.lines().count() as u32, ..Default::default() };
    let Some(tree) = parse(&tree_sitter_python::LANGUAGE.into(), src) else { ir.parse_error = true; return ir; };
    let root = tree.root_node();
    ir.parse_error = root.has_error();
    ir.env_keys = env_keys(src);
    ir.doc = module_doc(root, src);
    for c in named_children(root) { top(c, src, &mut ir, None); }
    ir
}

fn module_doc(root: Node, src: &str) -> String {
    for c in named_children(root) { if c.kind() == "expression_statement" { if let Some(s) = named_children(c).first() { if s.kind() == "string" { return clean_doc(text(*s, src)); } } } if c.kind() != "comment" { break; } }
    String::new()
}

fn clean_doc(s: &str) -> String { let t = s.trim().trim_matches(|c| c == '"' || c == '\'').trim(); let t: String = t.chars().take(600).collect(); t }

fn top(c: Node, src: &str, ir: &mut FileIR, class: Option<&str>) {
    match c.kind() {
        "import_statement" => for n in named_children(c) { match n.kind() { "dotted_name" => ir.imports.push(Import { spec: text(n, src).into(), names: vec![], line: line(c), namespace: Some(text(n, src).rsplit('.').next().unwrap_or("").into()), default: None }), "aliased_import" => { let name = field_text(n, "name", src); let alias = field_text(n, "alias", src); ir.imports.push(Import { spec: name.clone(), names: vec![], line: line(c), namespace: Some(if alias.is_empty() { name.rsplit('.').next().unwrap_or("").into() } else { alias }), default: None }); } _ => {} } },
        "import_from_statement" => {
            let module = field(c, "module_name").map(|m| text(m, src).to_string()).unwrap_or_default();
            let mut im = Import { spec: module, line: line(c), ..Default::default() };
            let mut first = true;
            for n in named_children(c) {
                if first { first = false; if n.kind() == "dotted_name" || n.kind() == "relative_import" { continue; } }
                match n.kind() { "dotted_name" => { let s = text(n, src).to_string(); im.names.push((s.clone(), s)); } "aliased_import" => { im.names.push((field_text(n, "name", src), field_text(n, "alias", src))); } "wildcard_import" => im.namespace = Some("*".into()), _ => {} }
            }
            ir.imports.push(im);
        }
        "function_definition" => ir.functions.push(func_of(c, src, class, vec![])),
        "decorated_definition" => {
            let decos: Vec<String> = named_children(c).iter().filter(|d| d.kind() == "decorator").map(|d| text(*d, src).trim_start_matches('@').to_string()).collect();
            if let Some(d) = field(c, "definition") { match d.kind() { "function_definition" => { let mut f = func_of(d, src, class, decos); f.line = line(c); ir.functions.push(f); } "class_definition" => class_of(d, src, ir, decos), _ => {} } }
        }
        "class_definition" => class_of(c, src, ir, vec![]),
        "expression_statement" | "if_statement" | "try_statement" | "with_statement" | "for_statement" | "while_statement" | "assignment" => { let mut f = Func::default(); collect_calls(c, src, &mut f); ir.top_calls.extend(f.calls); ir.top_strings.extend(f.strings); }
        _ => {}
    }
}

fn func_of(d: Node, src: &str, class: Option<&str>, decorators: Vec<String>) -> Func {
    let name = field_text(d, "name", src);
    let params = field_text(d, "parameters", src);
    let ret = field(d, "return_type").map(|r| format!(" -> {}", text(r, src))).unwrap_or_default();
    let is_async = text(d, src).starts_with("async");
    let mut f = Func { name: name.clone(), line: line(d), end_line: end_line(d), exported: !name.starts_with('_') || name == "__init__", kind: if class.is_some() { "method" } else { "fn" }.into(), class: class.map(|s| s.into()), is_async, decorators, ..Default::default() };
    f.signature = format!("{}{}{}", if is_async { "async " } else { "" }, params, ret);
    if let Some(body) = field(d, "body") {
        if let Some(first) = named_children(body).first() { if first.kind() == "expression_statement" { if let Some(s) = named_children(*first).first() { if s.kind() == "string" { f.doc = clean_doc(text(*s, src)); } } } }
        collect_calls(body, src, &mut f);
    }
    f.body = excerpt(src, f.line, f.end_line, 60);
    f
}

fn class_of(d: Node, src: &str, ir: &mut FileIR, decorators: Vec<String>) {
    let name = field_text(d, "name", src);
    let bases: Vec<String> = field(d, "superclasses").map(|s| named_children(s).iter().map(|b| text(*b, src).split('[').next().unwrap_or("").rsplit('.').next().unwrap_or("").to_string()).filter(|b| !b.is_empty()).collect()).unwrap_or_default();
    let kind = if bases.iter().any(|b| b == "Protocol") { "protocol" } else if bases.iter().any(|b| b.ends_with("Enum")) { "enum" } else if decorators.iter().any(|x| x.starts_with("dataclass")) { "dataclass" } else { "class" };
    let mut t = TypeDef { name: name.clone(), line: line(d), end_line: end_line(d), exported: !name.starts_with('_'), kind: kind.into(), ..Default::default() };
    t.extends = bases.iter().filter(|b| *b != "Protocol" && !b.ends_with("Enum") && *b != "object").cloned().collect();
    t.implements = bases.iter().filter(|b| *b == "Protocol").cloned().collect();
    if let Some(body) = field(d, "body") {
        if let Some(first) = named_children(body).first() { if first.kind() == "expression_statement" { if let Some(s) = named_children(*first).first() { if s.kind() == "string" { t.doc = clean_doc(text(*s, src)); } } } }
        for m in named_children(body) {
            match m.kind() {
                "function_definition" => { t.methods.push(field_text(m, "name", src)); ir.functions.push(func_of(m, src, Some(&name), vec![])); }
                "decorated_definition" => { if let Some(dd) = field(m, "definition") { if dd.kind() == "function_definition" { let decos: Vec<String> = named_children(m).iter().filter(|x| x.kind() == "decorator").map(|x| text(*x, src).trim_start_matches('@').to_string()).collect(); t.methods.push(field_text(dd, "name", src)); ir.functions.push(func_of(dd, src, Some(&name), decos)); } } }
                "expression_statement" => { t.refs.extend(super::ts::type_refs(text(m, src))); }
                _ => {}
            }
        }
    }
    t.refs.sort(); t.refs.dedup();
    t.text = excerpt(src, t.line, t.end_line, 40);
    ir.types.push(t);
}

fn collect_calls(n: Node, src: &str, f: &mut Func) {
    walk(n, &mut |c| {
        match c.kind() {
            "call" => { if let Some(func) = field(c, "function") { match func.kind() { "identifier" => f.calls.push(Call { name: text(func, src).into(), receiver: None, line: line(c), arg: first_string_arg(field(c, "arguments"), src) }), "attribute" => { let attr = field_text(func, "attribute", src); let obj = field(func, "object").map(|o| text(o, src).to_string()).unwrap_or_default(); f.calls.push(Call { name: attr, receiver: Some(obj.chars().take(120).collect()), line: line(c), arg: first_string_arg(field(c, "arguments"), src) }); } _ => {} } } true }
            "string" => { let v = string_value(c, src); if !v.is_empty() && v.len() < 400 { f.strings.push(v); } false }
            "function_definition" | "class_definition" => false,
            _ => true,
        }
    });
}
