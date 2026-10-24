//! TypeScript / TSX / JavaScript(tree-sitter)。import・export・関数(宣言・アロー・メソッド)・型(interface / type / class / enum)・呼び出し・文字列・directive。
use super::ir::*;
use super::ts_util::*;
use tree_sitter::Node;

pub fn extract(path: &str, lang: &str, src: &str) -> FileIR {
    let language = match lang { "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(), "js" => tree_sitter_javascript::LANGUAGE.into(), _ => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into() };
    let mut ir = FileIR { path: path.into(), lang: lang.into(), lines: src.lines().count() as u32, ..Default::default() };
    let Some(tree) = parse(&language, src) else { ir.parse_error = true; return ir; };
    let root = tree.root_node();
    ir.parse_error = root.has_error();
    ir.env_keys = env_keys(src);
    for c in named_children(root) {
        match c.kind() {
            "expression_statement" => {
                if let Some(s) = named_children(c).first() { if s.kind() == "string" { let v = string_value(*s, src); if v == "use server" || v == "use client" || v == "use cache" || v == "use workflow" { ir.directives.push(v); continue; } } }
                collect_top(c, src, &mut ir);
            }
            "import_statement" => ir.imports.push(import_of(c, src)),
            "export_statement" => {
                if let Some(d) = field(c, "declaration") { decl(d, src, true, &mut ir, None); }
                else {
                    // export { a, b as c } / export * from / export default expr
                    for ch in named_children(c) {
                        match ch.kind() {
                            "export_clause" => for sp in named_children(ch) { if sp.kind() == "export_specifier" { let local = field_text(sp, "name", src); let alias = field_text(sp, "alias", src); let n = if alias.is_empty() { local.clone() } else { alias }; if n != local { ir.aliases.push((n.clone(), local)); } ir.extra_exports.push(n); } },
                            "string" => { let spec = string_value(ch, src); ir.imports.push(Import { spec, names: vec![], line: line(c), namespace: Some("*".into()), default: None }); }
                            _ => {}
                        }
                    }
                    if text(c, src).starts_with("export default") { ir.extra_exports.push("default".into()); collect_top(c, src, &mut ir); }
                }
            }
            "function_declaration" | "generator_function_declaration" | "class_declaration" | "abstract_class_declaration" | "lexical_declaration" | "variable_declaration" | "interface_declaration" | "type_alias_declaration" | "enum_declaration" | "module" | "internal_module" => decl(c, src, false, &mut ir, None),
            "comment" => { if ir.functions.is_empty() && ir.doc.is_empty() && line(c) <= 3 { ir.doc = clean_comment(text(c, src)); } }
            _ => collect_top(c, src, &mut ir),
        }
    }
    ir
}

fn import_of(n: Node, src: &str) -> Import {
    let mut im = Import { spec: field(n, "source").map(|s| string_value(s, src)).unwrap_or_default(), line: line(n), ..Default::default() };
    for c in named_children(n) {
        if c.kind() == "import_clause" {
            for cc in named_children(c) {
                match cc.kind() {
                    "identifier" => im.default = Some(text(cc, src).into()),
                    "namespace_import" => im.namespace = named_children(cc).first().map(|i| text(*i, src).to_string()),
                    "named_imports" => for sp in named_children(cc) { if sp.kind() == "import_specifier" { let name = field_text(sp, "name", src); let alias = field_text(sp, "alias", src); let local = if alias.is_empty() { name.clone() } else { alias }; im.names.push((name, local)); } },
                    _ => {}
                }
            }
        }
    }
    im
}

fn decl(d: Node, src: &str, exported: bool, ir: &mut FileIR, class: Option<&str>) {
    match d.kind() {
        "function_declaration" | "generator_function_declaration" => ir.functions.push(func_of(d, src, exported, "fn", class, field_text(d, "name", src))),
        "lexical_declaration" | "variable_declaration" => {
            for v in named_children(d) {
                if v.kind() != "variable_declarator" { continue; }
                let name = field_text(v, "name", src);
                match field(v, "value") {
                    Some(val) if matches!(val.kind(), "arrow_function" | "function_expression" | "function" | "generator_function") => ir.functions.push(func_of(val, src, exported, "arrow", class, name)),
                    Some(val) if val.kind() == "identifier" => { ir.aliases.push((name.clone(), text(val, src).to_string())); if exported { ir.extra_exports.push(name); } }
                    Some(val) => {
                        // const x = wrap(handle) のような包み:引数の識別子を別名として取る(x → handle)
                        if val.kind() == "call_expression" { if let Some(args) = field(val, "arguments") { for a in named_children(args) { if a.kind() == "identifier" { ir.aliases.push((name.clone(), text(a, src).to_string())); break; } } } }
                        // const x = wrap(async () => {...}) のような高階の包み:中のアロー関数を名前付きで取る
                        let mut inner: Option<Node> = None;
                        walk(val, &mut |n| { if inner.is_none() && matches!(n.kind(), "arrow_function" | "function_expression") { inner = Some(n); false } else { n.kind() != "object" || true } });
                        if let Some(f) = inner { let mut fu = func_of(f, src, exported, "arrow", class, name.clone()); fu.decorators.push(first_ident(val, src)); fu.line = line(v); ir.functions.push(fu); }
                        else if exported { ir.extra_exports.push(name); let mut f = Func { name: String::new(), ..Default::default() }; collect_calls(val, src, &mut f, true); ir.top_calls.extend(f.calls); ir.top_strings.extend(f.strings); }
                        else { let mut f = Func::default(); collect_calls(val, src, &mut f, true); ir.top_calls.extend(f.calls); ir.top_strings.extend(f.strings); }
                    }
                    None => {}
                }
            }
        }
        "class_declaration" | "abstract_class_declaration" => {
            let name = field_text(d, "name", src);
            let mut t = TypeDef { name: name.clone(), line: line(d), end_line: end_line(d), exported, kind: "class".into(), doc: doc_before(d, src), ..Default::default() };
            for c in named_children(d) {
                if c.kind() == "class_heritage" {
                    for h in named_children(c) {
                        match h.kind() { "extends_clause" => t.extends.extend(named_children(h).iter().map(|x| base_name(text(*x, src)))), "implements_clause" => t.implements.extend(named_children(h).iter().map(|x| base_name(text(*x, src)))), _ => {} }
                    }
                }
            }
            if let Some(body) = field(d, "body") {
                for m in named_children(body) {
                    match m.kind() {
                        "method_definition" => { let mn = field_text(m, "name", src); t.methods.push(mn.clone()); ir.functions.push(func_of(m, src, exported, "method", Some(&name), mn)); }
                        "public_field_definition" | "property_signature" | "field_definition" => { if let Some(ty) = field(m, "type") { t.refs.extend(type_refs(text(ty, src))); } if let Some(val) = field(m, "value") { if matches!(val.kind(), "arrow_function" | "function_expression") { let mn = field_text(m, "name", src); t.methods.push(mn.clone()); ir.functions.push(func_of(val, src, exported, "method", Some(&name), mn)); } } }
                        _ => {}
                    }
                }
            }
            t.text = excerpt(src, t.line, t.end_line, 40);
            t.refs.sort(); t.refs.dedup();
            ir.types.push(t);
        }
        "interface_declaration" => {
            let mut t = TypeDef { name: field_text(d, "name", src), line: line(d), end_line: end_line(d), exported, kind: "interface".into(), doc: doc_before(d, src), ..Default::default() };
            for c in named_children(d) { if c.kind() == "extends_type_clause" { t.extends.extend(named_children(c).iter().map(|x| base_name(text(*x, src)))); } }
            if let Some(body) = field(d, "body") { t.refs = type_refs(text(body, src)); for m in named_children(body) { if m.kind() == "method_signature" { t.methods.push(field_text(m, "name", src)); } } }
            t.text = excerpt(src, t.line, t.end_line, 40);
            ir.types.push(t);
        }
        "type_alias_declaration" => {
            let mut t = TypeDef { name: field_text(d, "name", src), line: line(d), end_line: end_line(d), exported, kind: "type".into(), doc: doc_before(d, src), ..Default::default() };
            if let Some(v) = field(d, "value") { t.refs = type_refs(text(v, src)); }
            t.text = excerpt(src, t.line, t.end_line, 30);
            ir.types.push(t);
        }
        "enum_declaration" => { let mut t = TypeDef { name: field_text(d, "name", src), line: line(d), end_line: end_line(d), exported, kind: "enum".into(), doc: doc_before(d, src), ..Default::default() }; t.text = excerpt(src, t.line, t.end_line, 30); ir.types.push(t); }
        "module" | "internal_module" => { if let Some(body) = field(d, "body") { for c in named_children(body) { if c.kind() == "export_statement" { if let Some(dd) = field(c, "declaration") { decl(dd, src, true, ir, class); } } else { decl(c, src, false, ir, class); } } } }
        _ => collect_top(d, src, ir),
    }
}

fn first_ident(n: Node, src: &str) -> String {
    match n.kind() { "call_expression" => field(n, "function").map(|f| text(f, src).split('(').next().unwrap_or("").to_string()).unwrap_or_default(), _ => String::new() }
}

fn func_of(f: Node, src: &str, exported: bool, kind: &str, class: Option<&str>, name: String) -> Func {
    let params = field_text(f, "parameters", src);
    let ret = field(f, "return_type").map(|r| text(r, src).to_string()).unwrap_or_default();
    let sig_params = if params.len() > 160 { format!("{}…)", &params[..params.char_indices().nth(160).map(|(i, _)| i).unwrap_or(params.len())]) } else { params };
    let is_async = text(f, src).starts_with("async") || children(f).iter().any(|c| c.kind() == "async");
    let mut fu = Func { name, line: line(f), end_line: end_line(f), exported, kind: kind.into(), class: class.map(|s| s.to_string()), is_async, doc: doc_before(f, src), ..Default::default() };
    fu.signature = format!("{}{}{}", if is_async { "async " } else { "" }, sig_params, ret);
    if let Some(body) = field(f, "body") { collect_calls(body, src, &mut fu, false); fu.body = excerpt(src, fu.line, fu.end_line, 60); }
    else { collect_calls(f, src, &mut fu, false); fu.body = excerpt(src, fu.line, fu.end_line, 60); }
    fu
}

/// 関数の外側(モジュールレベル)の呼び出し・文字列を集める。
fn collect_top(n: Node, src: &str, ir: &mut FileIR) {
    let mut f = Func::default();
    collect_calls(n, src, &mut f, true);
    ir.top_calls.extend(f.calls); ir.top_strings.extend(f.strings);
}

/// 呼び出しと文字列。ネストした関数は(top の時だけ)境界で止めない:無名コールバック内の呼び出しも親の関数に帰属させる。
fn collect_calls(n: Node, src: &str, f: &mut Func, _top: bool) {
    walk(n, &mut |c| {
        match c.kind() {
            "call_expression" => {
                if let Some(func) = field(c, "function") {
                    match func.kind() {
                        "identifier" => f.calls.push(Call { name: text(func, src).into(), receiver: None, line: line(c), arg: first_string_arg(field(c, "arguments"), src) }),
                        "member_expression" => { let prop = field_text(func, "property", src); let obj = field(func, "object").map(|o| text(o, src).to_string()).unwrap_or_default(); let obj = if obj.len() > 120 { obj[..obj.char_indices().nth(120).map(|(i, _)| i).unwrap_or(obj.len())].to_string() } else { obj }; f.calls.push(Call { name: prop, receiver: Some(obj), line: line(c), arg: first_string_arg(field(c, "arguments"), src) }); }
                        "await_expression" | "parenthesized_expression" => {}
                        _ => {}
                    }
                }
                true
            }
            "new_expression" => { if let Some(ctor) = field(c, "constructor") { f.calls.push(Call { name: text(ctor, src).into(), receiver: Some("new".into()), line: line(c), arg: first_string_arg(field(c, "arguments"), src) }); } true }
            "string" | "template_string" => { let v = string_value(c, src); if !v.is_empty() && v.len() < 400 { f.strings.push(v); } true }
            "jsx_text" => false,
            _ => true,
        }
    });
}

fn base_name(s: &str) -> String { s.split('<').next().unwrap_or(s).trim().rsplit('.').next().unwrap_or(s).to_string() }

/// 型の本文から参照している型名(大文字始まりの識別子)を取る。
pub fn type_refs(s: &str) -> Vec<String> {
    static RX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"\b([A-Z][A-Za-z0-9_]{1,})\b").unwrap());
    let mut v: Vec<String> = RX.captures_iter(s).map(|c| c[1].to_string()).filter(|n| !BUILTIN.contains(&n.as_str())).collect();
    v.sort(); v.dedup(); v.truncate(40); v
}

pub const BUILTIN: &[&str] = &["Promise", "Record", "Partial", "Pick", "Omit", "Array", "Map", "Set", "Date", "Error", "String", "Number", "Boolean", "Object", "JSON", "Readonly", "Required", "Exclude", "Extract", "NonNullable", "ReturnType", "Parameters", "Awaited", "Request", "Response", "NextRequest", "NextResponse", "React", "ReactNode", "JSX", "Buffer", "URL", "Uint8Array", "Function", "Symbol", "Math", "Vec", "Option", "Result", "Box", "Arc", "Rc", "HashMap", "Self", "Some", "None", "Ok", "Err", "Default", "Clone", "Debug", "Send", "Sync", "List", "Dict", "Optional", "Any", "Union", "Tuple", "Callable", "Iterable", "Iterator", "Sequence", "Type", "TypeVar", "Generic", "ClassVar", "Final", "Literal", "True", "False", "Pick", "Keyof", "InstanceType", "HTMLElement", "Element", "Event", "FormData", "Headers", "Blob", "File", "ReadableStream", "AbortSignal", "Iterable", "AsyncIterable", "Generator", "Infer", "Zod", "ZodType", "Z", "T", "K", "V", "U", "P", "Props", "State", "Id", "Key"];
