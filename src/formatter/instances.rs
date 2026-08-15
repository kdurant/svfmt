//! 模块实例化与端口/参数连接对齐。

use crate::document::Doc;
use crate::formatter::expressions::{ExprCtx, fmt_default, fmt_expr};
use crate::formatter::tokens::display_width;
use crate::formatter::{Formatter, count_blank_lines};
use crate::parser::CstNode;

/// module_instantiation。
pub fn fmt_module_instantiation(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    if std::env::var("SVDBG").is_ok() {
        eprintln!("[MI] text={:?}", node.text().split('\n').next());
    }
    let items: Vec<CstNode<'_>> = node.children();
    let mut type_name = String::new();
    let mut params: Option<CstNode<'_>> = None;
    let mut instance: Option<CstNode<'_>> = None;
    let mut inst_name = String::new();
    let mut ports: Option<CstNode<'_>> = None;

    for c in &items {
        match c.kind() {
            "simple_identifier" if type_name.is_empty() => type_name = c.text().to_string(),
            "parameter_value_assignment" => params = Some(*c),
            "module_instance" | "hierarchical_instance" => instance = Some(*c),
            _ => {}
        }
    }
    if let Some(inst) = instance {
        let mut has_parens = false;
        for c in inst.children() {
            if (c.kind() == "name_of_instance" || c.kind() == "simple_identifier")
                && inst_name.is_empty()
            {
                inst_name = c.text().to_string();
            } else if c.kind() == "(" || c.kind() == ")" {
                has_parens = true;
            } else if c.kind() == "list_of_port_connections" {
                ports = Some(c);
            }
        }
        if ports.is_none() && has_parens {
            ports = Some(inst);
        }
    }

    // 接口实例化：压缩为一行
    if is_interface_type(f, &type_name) {
        return fmt_interface_one_line(f, node, &type_name, params, &inst_name, ports);
    }

    let mut docs: Vec<Doc> = Vec::new();
    // 参数与端口共享 name/value 对齐列
    let mut shared_name_max = 0usize;
    let mut shared_value_max = 0usize;
    if std::env::var("SVDBG").is_ok() {
        eprintln!("[inst2] type={:?}", type_name);
    }
    if let Some(p) = params {
        for c in parameter_connections_nodes(f, p) {
            let (name, value) = connection_columns(f, c);
            let w = display_width(&name, f.cfg.tab_width as usize);
            if w > shared_name_max {
                shared_name_max = w;
            }
            let vw = display_width(&value, f.cfg.tab_width as usize);
            if vw > shared_value_max {
                shared_value_max = vw;
            }
        }
    }
    if let Some(pl) = ports
        && pl.kind() == "list_of_port_connections"
    {
        for c in pl.children().into_iter().filter(|c| c.is_named()) {
            let (name, value) = connection_columns(f, c);
            let w = display_width(&name, f.cfg.tab_width as usize);
            if w > shared_name_max {
                shared_name_max = w;
            }
            let vw = display_width(&value, f.cfg.tab_width as usize);
            if vw > shared_value_max {
                shared_value_max = vw;
            }
        }
    }
    docs.push(Doc::text(type_name.clone()));
    if let Some(p) = params {
        docs.push(Doc::Space);
        docs.push(Doc::text("#"));
        if f.cfg.module.parameter_list_break_before_open_paren {
            docs.push(Doc::Newline);
        }
        docs.push(Doc::text("("));
        docs.push(Doc::Newline);
        docs.push(Doc::Indent);
        docs.push(fmt_parameter_connections(
            f,
            p,
            shared_name_max,
            shared_value_max,
        ));
        docs.push(Doc::Dedent);
        docs.push(Doc::Newline);
        docs.push(Doc::text(")"));
    }
    if params.is_none() && ports.is_none() {
        // 无参数无端口的简单实例：单行
        docs.push(Doc::Space);
        docs.push(Doc::text(inst_name.clone()));
        docs.push(Doc::text(";"));
        return Doc::concat(docs);
    }
    if params.is_none() {
        // 无参数：类型名与实例名同行
        docs.push(Doc::Space);
        docs.push(Doc::text(inst_name.clone()));
    } else {
        docs.push(Doc::Newline);
        docs.push(Doc::text(inst_name.clone()));
    }
    if let Some(pl) = ports {
        if f.cfg.module.instance_port_list_break_before_open_paren {
            docs.push(Doc::Newline);
        }
        docs.push(Doc::text("("));
        docs.push(Doc::Newline);
        docs.push(Doc::Indent);
        docs.push(fmt_port_connections(
            f,
            pl,
            shared_name_max,
            shared_value_max,
        ));
        docs.push(Doc::Dedent);
        docs.push(Doc::Newline);
        docs.push(Doc::text(")"));
    }
    docs.push(Doc::text(";"));
    Doc::concat(docs)
}

/// 判断类型名是否为接口（前缀/后缀命中）。
fn is_interface_type(f: &Formatter<'_>, type_name: &str) -> bool {
    if !f.cfg.one_line_interface_instantiation {
        return false;
    }
    let prefix = &f.cfg.interface_type_prefix;
    let suffix = &f.cfg.interface_type_suffix;
    (!prefix.is_empty() && type_name.starts_with(prefix.as_str()))
        || (!suffix.is_empty() && type_name.ends_with(suffix.as_str()))
}

/// 接口实例化单行输出。
fn fmt_interface_one_line(
    f: &Formatter<'_>,
    _node: CstNode<'_>,
    type_name: &str,
    params: Option<CstNode<'_>>,
    inst_name: &str,
    ports: Option<CstNode<'_>>,
) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    docs.push(Doc::text(type_name.to_string()));
    if let Some(p) = params {
        docs.push(Doc::Space);
        docs.push(Doc::text("#"));
        docs.push(Doc::text("("));
        docs.push(fmt_inline_connections(f, p));
        docs.push(Doc::text(")"));
    }
    docs.push(Doc::Space);
    docs.push(Doc::text(inst_name.to_string()));
    if let Some(pl) = ports {
        if pl.kind() == "list_of_port_connections" {
            docs.push(Doc::text("("));
            docs.push(fmt_inline_connections(f, pl));
            docs.push(Doc::text(")"));
        } else {
            // 空端口 `()`
            docs.push(Doc::text("()"));
        }
    }
    docs.push(Doc::text(";"));
    Doc::concat(docs)
}

/// 单行连接（接口压缩）：`.name(value)` 逗号空格分隔。
fn fmt_inline_connections(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let mut first = true;
    let prev_end: Option<usize> = None;
    for c in node.children() {
        if c.is_named() {
            if !first {
                docs.push(Doc::Space);
            }
            docs.push(fmt_named_connection(f, c));
            first = false;
        } else if c.kind() == "," {
            docs.push(Doc::text(","));
        }
        let _ = prev_end;
    }
    Doc::concat(docs)
}

/// 参数连接（`#(...)` 内，多行对齐）。
fn fmt_parameter_connections(
    f: &Formatter<'_>,
    node: CstNode<'_>,
    shared_name_max: usize,
    shared_value_max: usize,
) -> Doc {
    let items: Vec<CstNode<'_>> = node.children();
    let conns: Vec<CstNode<'_>> = items
        .iter()
        .filter(|c| c.kind() == "list_of_parameter_value_assignments")
        .flat_map(|l| {
            l.children()
                .into_iter()
                .filter(|c| c.is_named())
                .collect::<Vec<_>>()
        })
        .collect();
    aligned_connections(f, &conns, shared_name_max, shared_value_max, 0)
}

/// 端口连接（`(...)` 内，多行对齐）。
fn fmt_port_connections(
    f: &Formatter<'_>,
    node: CstNode<'_>,
    shared_name_max: usize,
    shared_value_max: usize,
) -> Doc {
    let items: Vec<CstNode<'_>> = node.children();
    let conns: Vec<CstNode<'_>> = items.iter().filter(|c| c.is_named()).copied().collect();
    aligned_connections(f, &conns, shared_name_max, shared_value_max, 0)
}

/// 对齐的连接列表。
fn aligned_connections(
    f: &Formatter<'_>,
    conns: &[CstNode<'_>],
    shared_name_max: usize,
    shared_value_max: usize,
    value_pad_extra: usize,
) -> Doc {
    if conns.is_empty() {
        return Doc::Nil;
    }
    let inner = f.cfg.space_inside_instance_port_parens as usize;
    let mut parsed: Vec<(String, String)> = Vec::new();
    for c in conns {
        let (name, value) = connection_columns(f, *c);
        if name.trim_end_matches('.').is_empty() {
            continue;
        }
        parsed.push((name, value));
    }
    let name_max = shared_name_max;
    let value_max = shared_value_max;
    if std::env::var("SVDBG").is_ok() {
        eprintln!(
            "[aligned2] name_max={} value_max={} extra={}",
            name_max, value_max, value_pad_extra
        );
    }
    let mut docs: Vec<Doc> = Vec::new();
    let mut rendered: Vec<(String, usize)> = Vec::new(); // (行文本, conns 索引)
    let mut pi = 0usize;
    for (ci, c) in conns.iter().enumerate() {
        if c.is_named() && c.kind().ends_with("comment") {
            // 行尾注释：追加到前一个连接行
            if let Some((line, _)) = rendered.last_mut() {
                line.push_str("  ");
                line.push_str(c.text());
            }
            continue;
        }
        if pi >= parsed.len() {
            break;
        }
        let (name, value) = &parsed[pi];
        let mut line = String::new();
        line.push_str(&pad_col(name, name_max + 1));
        line.push('(');
        line.push_str(&" ".repeat(inner));
        line.push_str(&pad_col(value, value_max + value_pad_extra));
        line.push_str(&" ".repeat(inner));
        line.push(')');
        let has_comma = pi + 1 < parsed.len() || c.text().trim_end().ends_with(',');
        if has_comma {
            line.push(',');
        }
        rendered.push((line, ci));
        pi += 1;
    }
    // 输出行，行间换行/空行
    let mut prev_ci: Option<usize> = None;
    for (idx, (line, ci)) in rendered.iter().enumerate() {
        if let Some(pci) = prev_ci {
            let ws = f.ws(conns[pci].byte_range().end, conns[*ci].byte_range().start);
            let blanks = count_blank_lines(ws);
            if blanks > 0 {
                docs.push(Doc::BlankLines(blanks));
            } else {
                docs.push(Doc::Newline);
            }
        }
        let _ = idx;
        docs.push(Doc::text(line.clone()));
        prev_ci = Some(*ci);
    }
    Doc::concat(docs)
}

fn pad_col(text: &str, width: usize) -> String {
    let w = display_width(text, 4);
    let mut out = String::from(text);
    if w < width {
        for _ in 0..(width - w) {
            out.push(' ');
        }
    }
    out
}

/// 提取连接的 name（`.port`）与 value（括号内表达式）。
fn connection_columns(f: &Formatter<'_>, node: CstNode<'_>) -> (String, String) {
    let _ = f;
    let items: Vec<CstNode<'_>> = node.children();
    let mut name = String::new();
    let mut value = String::new();
    let mut in_value = false;
    if std::env::var("SVDBG").is_ok() {
        eprintln!(
            "[conncols] node_kind={} items={:?}",
            node.kind(),
            items.iter().map(|c| c.kind()).collect::<Vec<_>>()
        );
    }
    for c in &items {
        match c.kind() {
            "named_port_connection" | "named_parameter_assignment" => {
                // 递归解析
                let sub: Vec<CstNode<'_>> = c.children();
                for s in &sub {
                    if s.kind() == "." {
                        continue;
                    }
                    if s.kind() == "simple_identifier" && name.is_empty() {
                        name = s.text().to_string();
                    } else if s.kind() == "param_expression"
                        || s.kind() == "expression"
                        || s.kind() == "mintypmax_expression"
                    {
                        let doc = fmt_expr(f, *s, &ExprCtx::default());
                        value = render_doc(f, doc);
                        in_value = true;
                    }
                }
            }
            "ordered_port_connection" | "ordered_parameter_assignment" => {
                let sub: Vec<CstNode<'_>> = c.children();
                for s in &sub {
                    if s.is_named() && s.kind() != "constant_expression" && s.kind() != "expression"
                    {
                        value = s.text().to_string();
                    }
                }
            }
            "." => {}
            "simple_identifier" if name.is_empty() => name = c.text().to_string(),
            "param_expression" | "expression" | "mintypmax_expression" => {
                let doc = fmt_expr(f, *c, &ExprCtx::default());
                value = render_doc(f, doc);
                in_value = true;
            }
            "(" | ")" => {}
            _ => {}
        }
    }
    let _ = in_value;
    (format!(".{name}"), value)
}

/// named 连接格式化（供内联使用）。
fn fmt_named_connection(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let _ = f;
    if node.kind() == "named_port_connection" || node.kind() == "named_parameter_assignment" {
        let (name, value) = connection_columns(f, node);
        let mut docs: Vec<Doc> = vec![Doc::text(name)];
        if !value.is_empty() {
            docs.push(Doc::text("("));
            docs.push(Doc::text(value));
            docs.push(Doc::text(")"));
        }
        Doc::concat(docs)
    } else {
        fmt_default(f, node)
    }
}

/// 供表达式使用的辅助。
pub fn _unused(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    fmt_expr(f, node, &ExprCtx::default())
}

/// 提取参数连接的节点列表。
fn parameter_connections_nodes<'a>(_f: &Formatter<'a>, node: CstNode<'a>) -> Vec<CstNode<'a>> {
    let items: Vec<CstNode<'a>> = node.children();
    items
        .iter()
        .filter(|c| c.kind() == "list_of_parameter_value_assignments")
        .flat_map(|l| {
            l.children()
                .into_iter()
                .filter(|c| c.is_named())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn render_doc(f: &Formatter<'_>, doc: crate::document::Doc) -> String {
    let opts = crate::document::RenderOptions::from(f.cfg);
    crate::document::render(&doc, &opts)
}
