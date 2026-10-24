//! インライン SVG の図。決定的なレイアウト。色は CSS 変数(ダーク対応)。
use super::html::esc;

pub fn role_color(r: &str) -> &'static str { match r { "entry" => "var(--l-entry)", "ui" => "var(--l-ui)", "application" => "var(--l-app)", "core" => "var(--l-core)", "adapter" => "var(--l-adapter)", "external" => "var(--l-ext)", "db" => "var(--l-db)", "ai" => "var(--l-ai)", "user" => "var(--l-user)", _ => "var(--l-other)" } }
pub fn role_ja(r: &str) -> &'static str { match r { "entry" => "entry", "ui" => "ui", "application" => "application", "core" => "core / domain", "adapter" => "adapter / infra", "external" => "external", "db" => "db", _ => "other" } }
pub const ROLE_ORDER: &[&str] = &["entry", "ui", "application", "core", "adapter", "other"];

/// n = 半角換算の文字数。全角(非 ASCII)は 1.9 文字分として数える。
fn short(s: &str, n: usize) -> String {
    let mut w = 0.0; let mut out = String::new(); let limit = n as f64;
    let total: f64 = s.chars().map(|c| if c.is_ascii() { 1.0 } else { 1.9 }).sum();
    if total <= limit { return s.into(); }
    for c in s.chars() { let cw = if c.is_ascii() { 1.0 } else { 1.9 }; if w + cw > limit - 1.0 { break; } w += cw; out.push(c); }
    out.push('…'); out
}
fn text(x: f64, y: f64, s: &str, cls: &str, anchor: &str, size: f64, extra: &str) -> String { format!("<text x=\"{x:.1}\" y=\"{y:.1}\" class=\"{cls}\" text-anchor=\"{anchor}\" font-size=\"{size}\" {extra}>{}</text>", esc(s)) }
pub fn id(s: &str) -> String { s.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>().trim_matches('-').to_string() }

pub struct Box_ { pub x: f64, pub y: f64, pub w: f64, pub h: f64, pub fill: String, pub title: String, pub sub: String, pub href: String, pub stroke: String, pub dashed: bool, pub sub2: String }
impl Default for Box_ { fn default() -> Self { Box_ { x: 0., y: 0., w: 150., h: 44., fill: "var(--l-other)".into(), title: String::new(), sub: String::new(), href: String::new(), stroke: String::new(), dashed: false, sub2: String::new() } } }

fn bx(b: &Box_) -> String {
    let st = if b.stroke.is_empty() { String::new() } else { format!(" stroke=\"{}\" stroke-width=\"2\"", b.stroke) };
    let dash = if b.dashed { " stroke-dasharray=\"5 4\"" } else { "" };
    let mut inner = format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"6\" fill=\"{}\" class=\"node\"{}{}/>", b.x, b.y, b.w, b.h, b.fill, st, dash);
    if !b.title.is_empty() { inner += &text(b.x + b.w / 2., b.y + if b.sub.is_empty() { b.h / 2. + 4. } else { b.h / 2. - 3. }, &short(&b.title, ((b.w / 7.2) as usize).max(6)), "node-title", "middle", 12.5, "font-weight=\"600\""); }
    if !b.sub.is_empty() { inner += &text(b.x + b.w / 2., b.y + b.h / 2. + 12., &short(&b.sub, ((b.w / 6.2) as usize).max(6)), "node-sub", "middle", 10.5, ""); }
    if !b.sub2.is_empty() { inner += &text(b.x + b.w / 2., b.y + b.h / 2. + 24., &short(&b.sub2, ((b.w / 6.2) as usize).max(6)), "node-sub", "middle", 10.5, ""); }
    let tip = format!("<title>{}{}</title>", esc(&b.title), if b.sub.is_empty() { String::new() } else { format!(" — {}", esc(&b.sub)) });
    if b.href.is_empty() { format!("<g>{}{}</g>", tip, inner) } else { format!("<a href=\"{}\" class=\"nodelink\">{}{}</a>", esc(&b.href), tip, inner) }
}

fn edge(x1: f64, y1: f64, x2: f64, y2: f64, label: &str, color: &str, width: f64, dashed: bool, marker: &str, curve: bool, both: bool) -> String {
    let dash = if dashed { " stroke-dasharray=\"6 4\"" } else { "" };
    let mk = format!(" marker-end=\"url(#{})\"{}", marker, if both { format!(" marker-start=\"url(#{}-start)\"", marker) } else { String::new() });
    let d = if curve { let dx = x2 - x1; format!("M{x1:.1},{y1:.1} C{:.1},{y1:.1} {:.1},{y2:.1} {x2:.1},{y2:.1}", x1 + dx * 0.45, x2 - dx * 0.45) } else { format!("M{x1:.1},{y1:.1} L{x2:.1},{y2:.1}") };
    let mut s = format!("<path d=\"{d}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{width:.1}\"{dash}{mk} class=\"edge\"/>");
    if !label.is_empty() { let (mx, my) = ((x1 + x2) / 2., (y1 + y2) / 2.); s += &format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"28\" height=\"16\" rx=\"8\" class=\"edge-label-bg\"/>", mx - 14., my - 9.); s += &text(mx, my + 4., label, "edge-label", "middle", 10.5, ""); }
    s
}

fn arc(x1: f64, y1: f64, x2: f64, y2: f64, bow: f64, label: &str, color: &str, width: f64, marker: &str, both: bool) -> String {
    let mx = (x1 + x2) / 2.; let cy = (y1 + y2) / 2. + bow;
    let mk = format!(" marker-end=\"url(#{})\"{}", marker, if both { format!(" marker-start=\"url(#{}-start)\"", marker) } else { String::new() });
    let mut s = format!("<path d=\"M{x1:.1},{y1:.1} Q{mx:.1},{cy:.1} {x2:.1},{y2:.1}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{width:.1}\"{mk} class=\"edge\"/>");
    if !label.is_empty() { let (lx, ly) = (0.25 * x1 + 0.5 * mx + 0.25 * x2, 0.25 * y1 + 0.5 * cy + 0.25 * y2); s += &format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"28\" height=\"16\" rx=\"8\" class=\"edge-label-bg\"/>", lx - 14., ly - 9.); s += &text(lx, ly + 4., label, "edge-label", "middle", 10.5, ""); }
    s
}

fn defs() -> &'static str { "<defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"9\" markerHeight=\"9\" markerUnits=\"userSpaceOnUse\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"var(--edge)\"/></marker><marker id=\"arrow-start\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"9\" markerHeight=\"9\" markerUnits=\"userSpaceOnUse\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"var(--edge)\"/></marker><marker id=\"arrow-red\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"9\" markerHeight=\"9\" markerUnits=\"userSpaceOnUse\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"var(--warn)\"/></marker><marker id=\"arrow-red-start\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"9\" markerHeight=\"9\" markerUnits=\"userSpaceOnUse\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"var(--warn)\"/></marker><marker id=\"tri\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"11\" markerHeight=\"11\" markerUnits=\"userSpaceOnUse\" orient=\"auto\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"var(--surface)\" stroke=\"var(--ink)\" stroke-width=\"1\"/></marker></defs>" }

fn svg(w: f64, h: f64, body: &str, stamp: &str) -> String { let st = if stamp.is_empty() { String::new() } else { text(w - 8., h - 8., stamp, "stamp", "end", 10.5, "") }; format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" width=\"100%\" class=\"diagram\" role=\"img\">{}{}{}</svg>", defs(), body, st) }

pub fn legend(items: &[(&str, &str)], x: f64, y: f64) -> String { let mut out = String::new(); let mut cx = x; for (color, label) in items { out += &format!("<rect x=\"{cx:.0}\" y=\"{:.0}\" width=\"12\" height=\"12\" rx=\"3\" fill=\"{color}\"/>", y - 9.); out += &text(cx + 17., y + 1., label, "legend", "start", 10.5, ""); cx += 17. + label.chars().count() as f64 * 7.5 + 18.; } out }

// ---------- 1. ハブ図 ----------
pub struct HubNode { pub title: String, pub sub: String, pub href: String, pub role: String, pub count: u32, pub main: bool, pub unknown: bool, pub both: bool }
impl Default for HubNode { fn default() -> Self { HubNode { title: String::new(), sub: String::new(), href: String::new(), role: "external".into(), count: 0, main: false, unknown: false, both: false } } }

pub fn hub(center: &[HubNode], left: &[HubNode], right: &[HubNode], bottom: &[HubNode], stamp: &str, tl: &str, tr: &str, tb: &str) -> String {
    let w = 960.; let (cw, ch) = (220., 64.); let (sw, sh, gap) = (180., 40., 10.);
    let n_side = left.len().max(right.len()).max(1) as f64;
    let center_h = center.len().max(1) as f64 * (ch + 12.);
    let body_h = (n_side * (sh + gap)).max(center_h) + 40.;
    let bottom_rows = if bottom.is_empty() { 0. } else { ((bottom.len() + 3) / 4) as f64 };
    let h = 40. + body_h + if bottom.is_empty() { 0. } else { bottom_rows * (sh + gap) + 50. } + 20.;
    let cx = w / 2. - cw / 2.; let cy0 = 40. + (body_h - 40. - center_h) / 2.;
    let mut out = String::new();
    out += &text(60., 24., tl, "col-title", "start", 12., ""); out += &text(w - 60., 24., tr, "col-title", "end", 12., ""); out += &text(w / 2., 24., "この repo", "col-title", "middle", 12., "");
    for (i, c) in center.iter().enumerate() { let y = cy0 + i as f64 * (ch + 12.); out += &bx(&Box_ { x: cx, y, w: cw, h: ch, fill: role_color(&c.role).into(), title: c.title.clone(), sub: c.sub.clone(), href: c.href.clone(), stroke: if c.main { "var(--ink)".into() } else { String::new() }, ..Default::default() }); }
    for (i, n) in left.iter().enumerate() { let y = 40. + i as f64 * (sh + gap); out += &bx(&Box_ { x: 60., y, w: sw, h: sh, fill: role_color(&n.role).into(), title: n.title.clone(), sub: n.sub.clone(), href: n.href.clone(), dashed: n.unknown, ..Default::default() }); out += &edge(60. + sw, y + sh / 2., cx, cy0 + ch / 2., &if n.count > 0 { n.count.to_string() } else { String::new() }, "var(--edge)", 1.5, false, "arrow", true, false); }
    let rx = w - 60. - sw;
    for (i, n) in right.iter().enumerate() { let y = 40. + i as f64 * (sh + gap); out += &bx(&Box_ { x: rx, y, w: sw, h: sh, fill: role_color(&n.role).into(), title: n.title.clone(), sub: n.sub.clone(), href: n.href.clone(), dashed: n.unknown, ..Default::default() }); out += &edge(cx + cw, cy0 + ch / 2., rx, y + sh / 2., &if n.count > 0 { n.count.to_string() } else { String::new() }, "var(--edge)", 1.5, false, "arrow", true, n.both); }
    if !bottom.is_empty() { let by0 = 40. + body_h + 10.; out += &text(w / 2., by0 - 2., tb, "col-title", "middle", 12., ""); let bw = 200.; for (i, n) in bottom.iter().enumerate() { let (r, c) = (i / 4, i % 4); let x = w / 2. - (bottom.len().min(4) as f64 * (bw + gap)) / 2. + c as f64 * (bw + gap); let y = by0 + 12. + r as f64 * (sh + gap); out += &bx(&Box_ { x, y, w: bw, h: sh, fill: role_color(&n.role).into(), title: n.title.clone(), sub: n.sub.clone(), href: n.href.clone(), ..Default::default() }); if r == 0 { out += &edge(cx + cw / 2., cy0 + center_h - 12., x + bw / 2., y, "", "var(--edge)", 1.5, false, "arrow", false, false); } } }
    svg(w, h, &out, stamp)
}

// ---------- 2. 層の図 ----------
pub struct LayerBox { pub path: String, pub role: String, pub files: u32, pub lines: u32, pub label: String, pub warn: bool }
pub struct LEdge { pub from: String, pub to: String, pub count: u32 }

pub fn layered(layers: &[LayerBox], edges: &[LEdge], viol: &[(String, String)], cycles: &[(String, String)], stamp: &str, href_prefix: &str, note: &str) -> String {
    let w = 1000.;
    let cols: Vec<&str> = ROLE_ORDER.iter().copied().filter(|r| layers.iter().any(|l| l.role == *r)).collect();
    if cols.is_empty() { return String::new(); }
    let colw = w / cols.len() as f64; let bw = (colw - 30.).min(170.); let (bh, gap) = (46., 14.); let max_per_col = 8;
    let mut by_col: std::collections::BTreeMap<&str, Vec<&LayerBox>> = std::collections::BTreeMap::new();
    for r in &cols { let mut v: Vec<&LayerBox> = layers.iter().filter(|l| l.role == *r).collect(); v.sort_by(|a, b| b.lines.cmp(&a.lines)); v.truncate(max_per_col); by_col.insert(r, v); }
    let role_of = |p: &str| layers.iter().find(|l| l.path == p).map(|l| l.role.as_str()).unwrap_or("other");
    let col_i = |r: &str| cols.iter().position(|c| *c == r).unwrap_or(cols.len() - 1);
    let has_back = edges.iter().any(|e| col_i(role_of(&e.from)) > col_i(role_of(&e.to)));
    let top = if has_back { 200. } else { 90. };
    let h = top + by_col.values().map(|v| v.len()).max().unwrap_or(1) as f64 * (bh + gap) + 190.;
    let mut pos: std::collections::HashMap<&str, (f64, f64)> = std::collections::HashMap::new();
    let mut out = text(8., 16., if note.is_empty() { "依存の向き:左(entry)→ 右(adapter)が正。正の向きの辺は箱の下を、逆向きの辺は上を回る。赤 = 禁止辺の違反か循環" } else { note }, "note", "start", 12., "");
    for (ci, r) in cols.iter().enumerate() {
        let x = ci as f64 * colw + (colw - bw) / 2.;
        out += &text(ci as f64 * colw + colw / 2., top - 12., role_ja(r), "col-title", "middle", 12., "");
        for (i, l) in by_col[r].iter().enumerate() { let y = top + i as f64 * (bh + gap); pos.insert(l.path.as_str(), (x, y)); out += &bx(&Box_ { x, y, w: bw, h: bh, fill: role_color(r).into(), title: if l.label.is_empty() { l.path.clone() } else { l.label.clone() }, sub: format!("{} files · {} 行", l.files, super::html::fmt_n(l.lines as u64)), href: format!("{}#layer-{}", href_prefix, id(&l.path)), stroke: if l.warn { "var(--warn)".into() } else { String::new() }, ..Default::default() }); }
        let hidden = layers.iter().filter(|l| l.role == *r).count().saturating_sub(max_per_col);
        if hidden > 0 { out += &text(ci as f64 * colw + colw / 2., top + by_col[r].len() as f64 * (bh + gap) + 8., &format!("ほか {} 層(表)", hidden), "note", "middle", 12., ""); }
    }
    let mut drawn: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut sorted: Vec<&LEdge> = edges.iter().collect(); sorted.sort_by(|a, b| b.count.cmp(&a.count));
    for e in sorted {
        let (Some(&(x1, y1)), Some(&(x2, y2))) = (pos.get(e.from.as_str()), pos.get(e.to.as_str())) else { continue };
        if drawn.contains(&(e.from.clone(), e.to.clone())) { continue; }
        let (ra, rb) = (role_of(&e.from), role_of(&e.to));
        let bad = viol.iter().any(|(a, b)| (a == ra && b == rb) || (a == &e.from && b == &e.to)) || cycles.iter().any(|(a, b)| (a == &e.from && b == &e.to) || (a == &e.to && b == &e.from));
        let (ia, ib) = (col_i(ra), col_i(rb));
        let width = 1. + ((e.count as f64 + 1.).log2() / 2.).min(2.5);
        let color = if bad { "var(--warn)" } else { "var(--edge)" }; let marker = if bad { "arrow-red" } else { "arrow" };
        let both = cycles.iter().any(|(a, b)| a == &e.to && b == &e.from) && !drawn.contains(&(e.to.clone(), e.from.clone()));
        if both { drawn.insert((e.to.clone(), e.from.clone())); }
        drawn.insert((e.from.clone(), e.to.clone()));
        let label = e.count.to_string();
        if ia == ib { out += &arc(x1 + bw, y1 + bh / 2., x2 + bw, y2 + bh / 2., 0., &label, color, width, marker, both); continue; }
        let dist = (ia as i64 - ib as i64).unsigned_abs() as f64;
        if ia < ib { let bow = if dist > 1. { (40. + 45. * dist).min(150.) } else { 26. }; out += &arc(x1 + bw * 0.75, y1 + bh, x2 + bw * 0.25, y2 + bh, bow, &label, color, width, marker, both); }
        else { let bow = if dist > 1. { -(40. + 45. * dist).min(150.) } else { -26. }; out += &arc(x1 + bw * 0.25, y1, x2 + bw * 0.75, y2, bow, &label, color, width, marker, both); }
    }
    let mut lg: Vec<(&str, &str)> = cols.iter().map(|r| (role_color(r), role_ja(r))).collect(); lg.push(("var(--warn)", "禁止辺・循環")); lg.push(("var(--edge)", "import の辺(数字 = 本数)"));
    out += &legend(&lg, 8., h - 18.);
    svg(w, h, &out, stamp)
}

// ---------- 3. ER 図 ----------
pub struct ErTable { pub name: String, pub columns: usize, pub fks: Vec<String>, pub rls: Option<bool>, pub rows: Option<i64>, pub readers: usize, pub writers: usize }
pub fn er(tables: &[ErTable], stamp: &str) -> String {
    let nodes: Vec<&ErTable> = tables.iter().take(15).collect();
    if nodes.is_empty() { return String::new(); }
    let w = 1000.; let h = if nodes.len() > 8 { 640. } else { 460. };
    let (cx, cy, rx, ry) = (w / 2., h / 2. - 10., w / 2. - 130., h / 2. - 70.); let (bw, bh) = (170., 46.);
    let n = nodes.len() as f64;
    let pos: std::collections::HashMap<&str, (f64, f64)> = nodes.iter().enumerate().map(|(i, t)| { let a = -std::f64::consts::FRAC_PI_2 + 2. * std::f64::consts::PI * i as f64 / n; (t.name.as_str(), (cx + rx * a.cos() - bw / 2., cy + ry * a.sin() - bh / 2.)) }).collect();
    let mut out = String::new();
    for t in &nodes { for f in &t.fks { if let (Some(&(x1, y1)), Some(&(x2, y2))) = (pos.get(t.name.as_str()), pos.get(f.as_str())) { if f != &t.name { out += &edge(x1 + bw / 2., y1 + bh / 2., x2 + bw / 2., y2 + bh / 2., "", "var(--edge)", 1.3, false, "arrow", false, false); } } } }
    for t in &nodes { let (x, y) = pos[t.name.as_str()]; let sub = format!("{} 列 · FK {}{} · R{} W{}", t.columns, t.fks.len(), t.rows.map(|r| format!(" · {} 行", super::html::fmt_n(r as u64))).unwrap_or_default(), t.readers, t.writers); out += &bx(&Box_ { x, y, w: bw, h: bh, fill: "var(--l-db)".into(), title: t.name.clone(), sub, href: format!("#table-{}", id(&t.name)), stroke: if t.rls == Some(false) { "var(--warn)".into() } else { String::new() }, ..Default::default() }); }
    out += &text(8., 16., &format!("中核 {} 表(FK の次数順)。矢印 = 外部キー(参照する → される)。赤枠 = RLS 無し。R/W = 読む関数 / 書く関数の数", nodes.len()), "note", "start", 12., "");
    out += &legend(&[("var(--l-db)", "表"), ("var(--warn)", "RLS 無し(赤枠)")], 8., h - 14.);
    svg(w, h, &out, stamp)
}

// ---------- 4. シーケンス図 ----------
pub struct Step { pub from: String, pub to: String, pub label: String, pub note: String }
pub fn sequence(steps: &[Step], title: &str, stamp: &str, role_of: &dyn Fn(&str) -> String) -> String {
    let mut lanes: Vec<&str> = Vec::new();
    for s in steps { for k in [s.from.as_str(), s.to.as_str()] { if !lanes.contains(&k) { lanes.push(k); } } }
    if lanes.is_empty() { return String::new(); }
    let w = (160. * lanes.len() as f64 + 80.).max(700.); let lw = (w - 80.) / lanes.len() as f64; let h = 90. + steps.len() as f64 * 46. + 30.;
    let mut out = if title.is_empty() { String::new() } else { text(10., 18., title, "col-title", "start", 12., "") };
    let xs: std::collections::HashMap<&str, f64> = lanes.iter().enumerate().map(|(i, l)| (*l, 40. + i as f64 * lw + lw / 2.)).collect();
    for l in &lanes { let x = xs[l]; out += &bx(&Box_ { x: x - 70., y: 30., w: 140., h: 36., fill: role_color(&role_of(l)).into(), title: l.to_string(), ..Default::default() }); out += &format!("<line x1=\"{x:.1}\" y1=\"66\" x2=\"{x:.1}\" y2=\"{:.1}\" stroke=\"var(--line)\" stroke-dasharray=\"3 4\"/>", h - 24.); }
    for (i, s) in steps.iter().enumerate() {
        let y = 90. + i as f64 * 46.; let (x1, x2) = (xs[s.from.as_str()], xs[s.to.as_str()]);
        if (x1 - x2).abs() < 1. { out += &format!("<path d=\"M{x1:.1},{y:.1} C{:.1},{y:.1} {:.1},{:.1} {x1:.1},{:.1}\" fill=\"none\" stroke=\"var(--edge)\" stroke-width=\"1.5\" marker-end=\"url(#arrow)\"/>", x1 + 50., x1 + 50., y + 22., y + 22.); out += &text(x1 + 56., y + 14., &short(&s.label, 40), "seq-label", "start", 11., ""); }
        else { out += &edge(x1, y, x2, y, "", "var(--edge)", 1.5, false, "arrow", false, false); out += &text((x1 + x2) / 2., y - 6., &short(&s.label, 44), "seq-label", "middle", 11., ""); }
        if !s.note.is_empty() { out += &text((x1 + x2) / 2., y + 13., &short(&s.note, 60), "note", "middle", 10., ""); }
    }
    svg(w, h, &out, stamp)
}

// ---------- 5. ツリーマップ ----------
fn squarify<'a, T>(items: &'a [(f64, T)], x: f64, y: f64, w: f64, h: f64) -> Vec<(&'a T, f64, f64, f64, f64)> {
    let items: Vec<&(f64, T)> = items.iter().filter(|(s, _)| *s > 0.).collect();
    let mut total: f64 = items.iter().map(|(s, _)| s).sum(); if total <= 0. { return vec![]; }
    let (mut x, mut y, mut w, mut h) = (x, y, w, h); let mut out = Vec::new(); let mut i = 0;
    while i < items.len() {
        let horiz = w >= h; let side = if horiz { h } else { w };
        let mut row: Vec<&(f64, T)> = Vec::new(); let mut best = f64::MAX; let mut j = i;
        while j < items.len() {
            row.push(items[j]); let rs: f64 = row.iter().map(|(s, _)| s).sum(); let frac = rs / total; let thick = (if horiz { w } else { h }) * frac;
            let worst = row.iter().map(|(s, _)| { let len = side * s / rs; (thick / (len + 1e-9)).max(len / (thick + 1e-9)) }).fold(0., f64::max);
            if worst > best && row.len() > 1 { row.pop(); break; } best = worst; j += 1;
        }
        let rs: f64 = row.iter().map(|(s, _)| s).sum(); let frac = rs / total;
        if horiz { let tw = w * frac; let mut cy = y; for (s, p) in &row { let th = h * (s / rs); out.push((p, x, cy, tw, th)); cy += th; } x += tw; w -= tw; } else { let th = h * frac; let mut cx = x; for (s, p) in &row { let tw = w * (s / rs); out.push((p, cx, y, tw, th)); cx += tw; } y += th; h -= th; }
        total -= rs; i += row.len();
    }
    out
}
pub struct TmLayer { pub path: String, pub lines: u32 }
pub struct TmFile { pub path: String, pub layer: String, pub count: u32, pub lines: u32, pub href: String }
pub fn treemap(layers: &[TmLayer], files: &[TmFile], stamp: &str) -> String {
    let (w, h) = (1000., 560.);
    let mut ls: Vec<&TmLayer> = layers.iter().collect(); ls.sort_by(|a, b| b.lines.cmp(&a.lines)); ls.truncate(14);
    let items: Vec<(f64, &TmLayer)> = ls.iter().map(|l| (l.lines as f64, *l)).collect();
    let rects = squarify(&items, 0., 24., w, h - 60.);
    let mut out = text(8., 16., "面積 = 行数、色の濃さ = 90 日の変更回数(層の合計)。中の箱 = 変更の多いファイル。赤枠 = 1,000 行超", "note", "start", 12., "");
    let heat_of = |p: &str| files.iter().filter(|f| f.layer == p).map(|f| f.count).sum::<u32>();
    let mx = ls.iter().map(|l| heat_of(&l.path)).max().unwrap_or(1).max(1) as f64;
    for (l, x, y, rw, rh) in rects {
        let heat = heat_of(&l.path) as f64 / mx;
        out += &format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"4\" fill=\"var(--heat)\" fill-opacity=\"{:.2}\" stroke=\"var(--line)\"/>", x + 2., y + 2., (rw - 4.).max(0.), (rh - 4.).max(0.), 0.08 + 0.72 * heat);
        if rw > 60. && rh > 24. { out += &text(x + 8., y + 16., &short(&format!("{} {}行", l.path, super::html::fmt_n(l.lines as u64)), (rw / 7.) as usize), "tm-title", "start", 11.5, "font-weight=\"600\""); }
        let mut inner: Vec<&TmFile> = files.iter().filter(|f| f.layer == l.path).collect(); inner.sort_by(|a, b| b.count.cmp(&a.count)); inner.truncate(6);
        if !inner.is_empty() && rw > 120. && rh > 70. {
            let it: Vec<(f64, &TmFile)> = inner.iter().map(|f| (f.lines.max(50) as f64, *f)).collect();
            for (f, fx, fy, fw, fh) in squarify(&it, x + 8., y + 24., rw - 16., rh - 32.) {
                if fw < 18. || fh < 14. { continue; }
                let stroke = if f.lines >= 1000 { " stroke=\"var(--warn)\" stroke-width=\"2\"" } else { " stroke=\"var(--surface)\"" };
                out += &format!("<a href=\"{}\"><title>{} — {} 回 / {} 行</title><rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"3\" fill=\"var(--surface)\" fill-opacity=\"0.75\"{}/>", esc(&f.href), esc(&f.path), f.count, f.lines, fx + 1., fy + 1., fw - 2., fh - 2., stroke);
                if fw > 50. && fh > 26. { out += &text(fx + fw / 2., fy + fh / 2. + 4., &short(f.path.rsplit('/').next().unwrap_or(""), (fw / 6.5) as usize), "tm-file", "middle", 10.5, ""); }
                out += "</a>";
            }
        }
    }
    out += &legend(&[("var(--heat)", "変更の多さ(濃い = 多い)"), ("var(--warn)", "1,000 行超(赤枠)")], 8., h - 12.);
    svg(w, h, &out, stamp)
}

// ---------- 6. 週次 ----------
pub fn weekly_bars(weekly: &[crate::analyze::Week], stamp: &str) -> String {
    if weekly.is_empty() { return String::new(); }
    let (w, h) = (1000., 260.);
    let mut all: std::collections::BTreeMap<&str, u32> = std::collections::BTreeMap::new();
    for wk in weekly { for (l, n) in &wk.layers { *all.entry(l).or_insert(0) += n; } }
    let mut top: Vec<(&str, u32)> = all.into_iter().collect(); top.sort_by(|a, b| b.1.cmp(&a.1)); top.truncate(6);
    let palette = ["var(--l-entry)", "var(--l-ui)", "var(--l-app)", "var(--l-core)", "var(--l-adapter)", "var(--l-other)"];
    let mx = weekly.iter().map(|wk| wk.layers.values().sum::<u32>().max(wk.commits)).max().unwrap_or(1).max(1) as f64;
    let bw = (w - 80.) / weekly.len() as f64;
    let mut out = text(8., 16., "週ごとの変更ファイル数(層で色分け)。数字 = commit 数", "note", "start", 12., "");
    for (i, wk) in weekly.iter().enumerate() {
        let x = 50. + i as f64 * bw; let mut y = h - 50.;
        for (j, (l, _)) in top.iter().enumerate() { let n = *wk.layers.get(*l).unwrap_or(&0); if n == 0 { continue; } let bh = (h - 100.) * n as f64 / mx; out += &format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"{}\"><title>{}: {}</title></rect>", x + 6., y - bh, bw - 12., bh, palette[j], esc(l), n); y -= bh; }
        let other: u32 = wk.layers.iter().filter(|(l, _)| !top.iter().any(|(t, _)| t == &l.as_str())).map(|(_, n)| n).sum();
        if other > 0 { let bh = (h - 100.) * other as f64 / mx; out += &format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"var(--l-other)\"><title>other: {}</title></rect>", x + 6., y - bh, bw - 12., bh, other); y -= bh; }
        out += &text(x + bw / 2., h - 36., &wk.week[5..], "axis", "middle", 10., "");
        if wk.commits > 0 { out += &text(x + bw / 2., y - 4., &wk.commits.to_string(), "axis", "middle", 10., ""); }
    }
    let mut lg: Vec<(&str, &str)> = top.iter().enumerate().map(|(i, (l, _))| (palette[i], *l)).collect(); lg.push(("var(--l-other)", "other"));
    out += &legend(&lg, 8., h - 10.);
    svg(w, h, &out, stamp)
}

// ---------- 7. 時間線 ----------
pub fn timeline(dates: &[String], total: usize, stamp: &str) -> String {
    let dated: Vec<&String> = dates.iter().filter(|d| !d.is_empty()).collect();
    if dated.is_empty() { return String::new(); }
    let mut months: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    for d in &dated { *months.entry(d[..7].to_string()).or_insert(0) += 1; }
    let keys: Vec<&String> = months.keys().collect();
    let (y0, m0) = (keys[0][..4].parse::<i32>().unwrap_or(2020), keys[0][5..7].parse::<i32>().unwrap_or(1));
    let (y1, m1) = (keys[keys.len() - 1][..4].parse::<i32>().unwrap_or(2020), keys[keys.len() - 1][5..7].parse::<i32>().unwrap_or(1));
    let mut all: Vec<String> = Vec::new(); let (mut y, mut m) = (y0, m0);
    while (y, m) <= (y1, m1) { all.push(format!("{:04}-{:02}", y, m)); m += 1; if m > 12 { m = 1; y += 1; } }
    if all.len() > 36 { all = all[all.len() - 36..].to_vec(); }
    let (w, h) = (1000., 170.); let bw = (w - 60.) / all.len().max(1) as f64; let mx = *months.values().max().unwrap_or(&1) as f64;
    let mut out = text(8., 16., &format!("migration の本数(月ごと)。全 {} 本、{} 〜 {}", total, dated.iter().min().map(|s| s.as_str()).unwrap_or(""), dated.iter().max().map(|s| s.as_str()).unwrap_or("")), "note", "start", 12., "");
    let step = (all.len() / 12).max(1);
    for (i, k) in all.iter().enumerate() { let n = *months.get(k).unwrap_or(&0); let bh = (h - 80.) * n as f64 / mx; let x = 40. + i as f64 * bw; out += &format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"var(--l-db)\"><title>{}: {} 本</title></rect>", x + 2., h - 45. - bh, (bw - 4.).max(1.), bh, k, n); if n > 0 { out += &text(x + bw / 2., h - 48. - bh, &n.to_string(), "axis", "middle", 9.5, ""); } if i % step == 0 { out += &text(x + bw / 2., h - 30., k, "axis", "middle", 9.5, ""); } }
    svg(w, h, &out, stamp)
}

// ---------- 8. 行列 ----------
pub fn matrix(rows: &[String], cols: &[String], cell: &dyn Fn(&str, &str) -> (String, String), note: &str, stamp: &str) -> String {
    let (rw, cw, ch) = (170., 110., 34.); let w = rw + cw * cols.len() as f64 + 40.; let h = 40. + ch * (rows.len() + 1) as f64 + 30.;
    let mut out = if note.is_empty() { String::new() } else { text(8., 16., note, "note", "start", 12., "") };
    for (j, c) in cols.iter().enumerate() { out += &text(rw + 20. + j as f64 * cw + cw / 2., 50., c, "col-title", "middle", 11.5, ""); }
    for (i, r) in rows.iter().enumerate() { let y = 60. + i as f64 * ch; out += &text(rw + 10., y + ch / 2. + 4., &short(r, 24), "row-title", "end", 11.5, ""); for (j, c) in cols.iter().enumerate() { let (v, fill) = cell(r, c); let x = rw + 20. + j as f64 * cw; out += &format!("<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"4\" fill=\"{}\"/>", x + 2., y + 2., cw - 4., ch - 4., fill); out += &text(x + cw / 2., y + ch / 2. + 4., &v, "cell", "middle", 12., ""); } }
    svg(w, h, &out, stamp)
}

// ---------- 9. パイプライン ----------
pub fn pipeline(stages: &[(String, String, Option<bool>)], stamp: &str) -> String {
    let w = 1000.; let n = stages.len().max(1) as f64; let bw = ((w - 40.) / n - 20.).min(160.); let bh = 56.; let h = 120.;
    let mut out = String::new();
    for (i, (title, sub, ok)) in stages.iter().enumerate() { let x = 20. + i as f64 * ((w - 40.) / n) + ((w - 40.) / n - bw) / 2.; let fill = match ok { Some(true) => "var(--l-app)", Some(false) => "var(--warn-soft)", None => "var(--code)" }; out += &bx(&Box_ { x, y: 30., w: bw, h: bh, fill: fill.into(), title: title.clone(), sub: sub.clone(), dashed: ok.is_none(), ..Default::default() }); if (i as f64) < n - 1. { out += &edge(x + bw, 30. + bh / 2., x + bw + ((w - 40.) / n - bw) - 2., 30. + bh / 2., "", "var(--edge)", 1.5, false, "arrow", false, false); } }
    out += &legend(&[("var(--l-app)", "ある"), ("var(--code)", "無し(点線)")], 8., h - 10.);
    svg(w, h, &out, stamp)
}

// ---------- 10. クラス図 ----------
pub struct ClassNode { pub name: String, pub kind: String, pub file: String, pub role: String, pub extends: Vec<String>, pub implements: Vec<String>, pub refs: Vec<String>, pub referenced: u32, pub href: String }
pub fn classdiag(nodes: &[ClassNode], stamp: &str) -> String {
    let nodes: Vec<&ClassNode> = nodes.iter().take(30).collect();
    if nodes.is_empty() { return String::new(); }
    let cols = if nodes.len() > 12 { 5 } else { 4 }; let (bw, bh, gx, gy) = (170., 52., 30., 46.); let w = cols as f64 * (bw + gx) + 40.; let rows = (nodes.len() + cols - 1) / cols; let h = 30. + rows as f64 * (bh + gy) + 40.;
    let pos: std::collections::HashMap<&str, (f64, f64)> = nodes.iter().enumerate().map(|(i, n)| (n.name.as_str(), (20. + (i % cols) as f64 * (bw + gx), 30. + (i / cols) as f64 * (bh + gy)))).collect();
    let mut out = text(8., 16., "白抜き矢印 = 継承 / 破線 = 実装(trait・Protocol・interface) / 細線 = 型の参照。箱の色 = 層", "note", "start", 12., "");
    for n in &nodes { let (x1, y1) = pos[n.name.as_str()]; for t in &n.extends { if let Some(&(x2, y2)) = pos.get(t.as_str()) { out += &format!("<path d=\"M{:.1},{:.1} L{:.1},{:.1}\" fill=\"none\" stroke=\"var(--ink)\" stroke-width=\"1.5\" marker-end=\"url(#tri)\"/>", x1 + bw / 2., y1, x2 + bw / 2., y2 + bh); } } for t in &n.implements { if let Some(&(x2, y2)) = pos.get(t.as_str()) { out += &format!("<path d=\"M{:.1},{:.1} L{:.1},{:.1}\" fill=\"none\" stroke=\"var(--ink)\" stroke-width=\"1.2\" stroke-dasharray=\"5 4\" marker-end=\"url(#tri)\"/>", x1 + bw / 2., y1, x2 + bw / 2., y2 + bh); } } for t in n.refs.iter().take(4) { if let Some(&(x2, y2)) = pos.get(t.as_str()) { if !n.extends.contains(t) && !n.implements.contains(t) { out += &format!("<path d=\"M{:.1},{:.1} L{:.1},{:.1}\" fill=\"none\" stroke=\"var(--edge)\" stroke-width=\"0.8\" stroke-opacity=\"0.6\"/>", x1 + bw / 2., y1 + bh / 2., x2 + bw / 2., y2 + bh / 2.); } } } }
    for n in &nodes { let (x, y) = pos[n.name.as_str()]; out += &bx(&Box_ { x, y, w: bw, h: bh, fill: role_color(&n.role).into(), title: format!("«{}» {}", n.kind, n.name), sub: n.file.rsplit('/').next().unwrap_or("").into(), href: n.href.clone(), sub2: format!("被参照 {}", n.referenced), ..Default::default() }); }
    svg(w, h, &out, stamp)
}

// ---------- 11. 入口の地図 ----------
pub fn entry_map(counts: &[(String, Vec<(String, u32)>)], stamp: &str) -> String {
    // counts: [(kind, [(auth, n)])]
    if counts.is_empty() { return String::new(); }
    let w = 1000.; let colw = w / counts.len() as f64; let h = 230.;
    fn ja(k: &str) -> String { match k { "http" => "HTTP route".into(), "server_action" => "server action".into(), "webhook" => "webhook".into(), "cron" => "cron".into(), "queue" => "queue consumer".into(), "cli" => "CLI / main".into(), o => o.to_string() } }
    let col = |a: &str| match a { "middleware" => "var(--ok)", "inline" => "var(--l-app)", "none" => "var(--warn)", _ => "var(--l-other)" };
    let mut out = text(8., 16., "入口の種類ごとの本数。色 = 認可の取り方(緑 middleware / 青 handler かその import / 赤 無し / 灰 不明)", "note", "start", 12., "");
    for (i, (k, auths)) in counts.iter().enumerate() { let x = i as f64 * colw + 20.; let total: u32 = auths.iter().map(|a| a.1).sum(); out += &text(x + (colw - 40.) / 2., 44., &format!("{}  {}", ja(k), total), "col-title", "middle", 12., ""); let mut y = 60.; for (a, n) in auths { if *n == 0 { continue; } let bh = 120. * *n as f64 / total.max(1) as f64; out += &format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"{}\" fill-opacity=\"0.85\"><title>{}: {}</title></rect>", x, y, colw - 40., bh, col(a), a, n); if bh > 14. { out += &text(x + (colw - 40.) / 2., y + bh / 2. + 4., &format!("{} {}", a, n), "cell", "middle", 11., ""); } y += bh; } }
    out += &legend(&[("var(--ok)", "middleware"), ("var(--l-app)", "handler で確認"), ("var(--warn)", "認可なし"), ("var(--l-other)", "不明")], 8., h - 12.);
    svg(w, h, &out, stamp)
}

// ---------- 12. cron の時刻 ----------
pub fn cron_clock(crons: &[(String, String, String)], stamp: &str) -> String {
    // (path, schedule, auth)
    if crons.is_empty() { return String::new(); }
    let (w, h) = (1000., 150.);
    let mut hours: std::collections::BTreeMap<u32, Vec<&(String, String, String)>> = std::collections::BTreeMap::new();
    for c in crons { let p: Vec<&str> = c.1.split_whitespace().collect(); if p.len() != 5 { continue; } let hs: Vec<u32> = if let Ok(n) = p[1].parse::<u32>() { vec![n] } else if let Some(s) = p[1].strip_prefix("*/") { s.parse::<u32>().map(|k| (0..24).step_by(k.max(1) as usize).collect()).unwrap_or_default() } else if p[1] == "*" { (0..24).collect() } else { p[1].split(',').filter_map(|x| x.parse().ok()).collect() }; for hh in hs { hours.entry(hh).or_default().push(c); } }
    let bw = (w - 60.) / 24.; let mx = hours.values().map(|v| v.len()).max().unwrap_or(1).max(1) as f64;
    let mut out = text(8., 16., "cron の発火時刻(UTC、時間ごと)。箱 1 つ = 1 本。赤 = 認可なし", "note", "start", 12., "");
    for hh in 0..24u32 { let x = 30. + hh as f64 * bw; for (i, c) in hours.get(&hh).into_iter().flatten().enumerate() { let fill = if c.2 == "none" { "var(--warn)" } else { "var(--l-app)" }; out += &format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"{}\"><title>{} ({})</title></rect>", x + 3., h - 45. - (i as f64 + 1.) * (80. / mx), bw - 6., 80. / mx - 2., fill, esc(&c.0), esc(&c.1)); } out += &text(x + bw / 2., h - 30., &hh.to_string(), "axis", "middle", 10., ""); }
    svg(w, h, &out, stamp)
}

// ---------- 13. call graph(入口から深さごとに並べる) ----------
pub struct CgNode { pub id: String, pub name: String, pub layer: String, pub role: String, pub depth: u32, pub href: String, pub kind: String }
pub fn callgraph(nodes: &[CgNode], edges: &[(String, String)], tables: &[(String, String, bool)], externals: &[(String, String)], stamp: &str) -> String {
    // tables: (fn id, table, write) / externals: (fn id, name)
    if nodes.is_empty() { return String::new(); }
    let maxd = nodes.iter().map(|n| n.depth).max().unwrap_or(0) as usize;
    let mut cols: Vec<Vec<&CgNode>> = vec![vec![]; maxd + 1];
    for n in nodes { cols[n.depth as usize].push(n); }
    for c in cols.iter_mut() { c.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.name.cmp(&b.name))); c.truncate(10); }
    let extra_cols = if !tables.is_empty() || !externals.is_empty() { 1 } else { 0 };
    let ncols = maxd + 1 + extra_cols; let (bw, bh, gy) = (160., 40., 12.); let colw = bw + 60.; let w = (ncols as f64 * colw + 40.).max(600.);
    let rows = cols.iter().map(|c| c.len()).max().unwrap_or(1).max(tables.len().min(10) + externals.len().min(6)) as f64;
    let h = 60. + rows * (bh + gy) + 30.;
    let mut pos: std::collections::HashMap<&str, (f64, f64)> = std::collections::HashMap::new();
    let mut out = text(8., 16., "入口から呼び出しを辿った順(左 → 右 = 呼び出しの深さ)。箱の色 = 層。右端 = 読み書きする表と外部", "note", "start", 12., "");
    for (d, c) in cols.iter().enumerate() { let x = 20. + d as f64 * colw; out += &text(x + bw / 2., 40., &if d == 0 { "handler".into() } else { format!("depth {}", d) }, "col-title", "middle", 11.5, ""); for (i, n) in c.iter().enumerate() { let y = 52. + i as f64 * (bh + gy); pos.insert(n.id.as_str(), (x, y)); out += &bx(&Box_ { x, y, w: bw, h: bh, fill: role_color(&n.role).into(), title: n.name.clone(), sub: n.layer.clone(), href: n.href.clone(), ..Default::default() }); } }
    for (a, b) in edges { if let (Some(&(x1, y1)), Some(&(x2, y2))) = (pos.get(a.as_str()), pos.get(b.as_str())) { if x2 > x1 { out += &edge(x1 + bw, y1 + bh / 2., x2, y2 + bh / 2., "", "var(--edge)", 1.2, false, "arrow", true, false); } else { out += &arc(x1 + bw / 2., y1 + bh, x2 + bw / 2., y2 + bh, 30., "", "var(--edge)", 1., "arrow", false); } } }
    if extra_cols == 1 {
        let x = 20. + (maxd + 1) as f64 * colw; out += &text(x + bw / 2., 40., "表 / 外部", "col-title", "middle", 11.5, "");
        let mut i = 0; let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (f, t, wr) in tables { if seen.contains(t) { if let Some(&(x1, y1)) = pos.get(f.as_str()) { if let Some(&(x2, y2)) = pos.get(t.as_str()) { out += &edge(x1 + bw, y1 + bh / 2., x2, y2 + bh / 2., "", if *wr { "var(--warn)" } else { "var(--edge)" }, 1.2, *wr, if *wr { "arrow-red" } else { "arrow" }, true, false); } } continue; } if i >= 10 { break; } let y = 52. + i as f64 * (bh + gy); seen.insert(t.clone()); pos.insert(t.as_str(), (x, y)); out += &bx(&Box_ { x, y, w: bw, h: bh, fill: "var(--l-db)".into(), title: t.clone(), sub: if *wr { "write".into() } else { "read".into() }, href: format!("{}#table-{}", super::html::file_of("03"), id(t)), ..Default::default() }); if let Some(&(x1, y1)) = pos.get(f.as_str()) { out += &edge(x1 + bw, y1 + bh / 2., x, y + bh / 2., "", if *wr { "var(--warn)" } else { "var(--edge)" }, 1.2, *wr, if *wr { "arrow-red" } else { "arrow" }, true, false); } i += 1; }
        for (f, e) in externals.iter().take(6) { if seen.contains(e) { continue; } let y = 52. + i as f64 * (bh + gy); seen.insert(e.clone()); out += &bx(&Box_ { x, y, w: bw, h: bh, fill: if e == "LLM" { "var(--l-ai)".into() } else { "var(--l-ext)".into() }, title: e.clone(), sub: "external".into(), href: super::html::file_of("12").into(), ..Default::default() }); if let Some(&(x1, y1)) = pos.get(f.as_str()) { out += &edge(x1 + bw, y1 + bh / 2., x, y + bh / 2., "", "var(--edge)", 1.2, false, "arrow", true, false); } i += 1; }
    }
    out += &legend(&[("var(--l-entry)", "entry"), ("var(--l-app)", "application"), ("var(--l-core)", "core"), ("var(--l-adapter)", "adapter"), ("var(--l-db)", "表(赤 = write)"), ("var(--l-ext)", "外部")], 8., h - 10.);
    svg(w, h, &out, stamp)
}
