//! 模块头与参数列表布局。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::formatter::alignment::pad_to;
use crate::formatter::count_blank_lines;
use crate::formatter::tokens::{display_width, has_newline};
use crate::parser::CstNode;

use super::pad_comment;
use super::ports::fmt_port_declarations;

/// module_ansi_header / module_nonansi_header。
pub(crate) fn fmt_module_header(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();
    let mut i = 0;
    // 是否位于行首：docs 末尾是 Newline（或 docs 为空），下一输出将顶格。
    let mut at_line_start = true;
    // module + name
    while i < items.len() {
        let c = items[i];
        if c.kind() == "module_keyword" {
            docs.push(Doc::text("module"));
            at_line_start = false;
            i += 1;
        } else if c.kind() == "simple_identifier" {
            if !at_line_start {
                docs.push(Doc::Space);
            }
            docs.push(Doc::text(c.text()));
            at_line_start = false;
            i += 1;
        } else {
            break;
        }
    }
    // 头部其余部分（parameter/port/package_import 等）
    let mut port_paren_break = false;
    let mut seen_semi = false;
    while i < items.len() {
        let c = items[i];
        match c.kind() {
            "package_import_declaration" => {
                // import 与模块名同行（空格分隔），避免粘连成一个标识符。
                // 粘连会把 `module nameimport ...` 拼成一个标识符，导致二次
                // 解析结构漂移、破坏幂等性（见 examples/bsg.sv）。
                if !at_line_start {
                    docs.push(Doc::Space);
                }
                docs.push(f.fmt(c));
                at_line_start = false;
                i += 1;
            }
            "parameter_port_list" => {
                if !at_line_start {
                    docs.push(Doc::Space);
                }
                docs.push(fmt_parameter_port_list(f, c));
                at_line_start = false;
                i += 1;
            }
            "list_of_port_declarations" => {
                // 括号换行配置
                if f.cfg.module.port_list_break_before_open_paren {
                    docs.push(Doc::Newline);
                    at_line_start = true;
                } else {
                    if !at_line_start {
                        docs.push(Doc::Space);
                    }
                    at_line_start = false;
                }
                docs.push(Doc::text("("));
                docs.push(Doc::Newline);
                docs.push(Doc::Indent);
                let ports = fmt_port_declarations(f, c);
                docs.push(ports);
                docs.push(Doc::Dedent);
                docs.push(Doc::Newline);
                docs.push(Doc::text(")"));
                port_paren_break = true;
                i += 1;
            }
            "list_of_ports" => {
                // 非 ANSI：空则 `()`，否则 `(...)`，紧跟模块名（无空格）
                let text = c.text();
                docs.push(Doc::text(text.trim_end().to_string()));
                at_line_start = false;
                i += 1;
            }
            ";" => {
                docs.push(Doc::text(";"));
                seen_semi = true;
                i += 1;
            }
            _ => {
                docs.push(f.fmt(c));
                i += 1;
            }
        }
    }
    let _ = port_paren_break;
    let _ = seen_semi;
    Doc::concat(docs)
}

/// parameter_port_list：`#(...)`。
pub(crate) fn fmt_parameter_port_list(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    // align_parameters 关闭时：保持原样，不做列对齐
    if !f.cfg.module.align_parameters {
        return Doc::text(node.text().trim_end().to_string());
    }
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();
    // `#` 与 `(`
    docs.push(Doc::text("#"));
    if f.cfg.module.parameter_list_break_before_open_paren {
        docs.push(Doc::Newline);
    }
    docs.push(Doc::text("("));
    docs.push(Doc::Newline);
    docs.push(Doc::Indent);
    // 参数行对齐
    let params: Vec<CstNode<'_>> = items
        .iter()
        .filter(|c| c.kind() == "parameter_port_declaration")
        .copied()
        .collect();
    // 收集 (prefix, name, value, 尾随逗号)
    let mut rows: Vec<(String, String, String)> = Vec::new();
    let mut has_comma: Vec<bool> = Vec::new();
    for p in &params {
        let (prefix, cols) = parameter_columns(f, *p);
        rows.push((prefix, cols[0].clone(), cols[1].clone()));
        let text = p.text();
        has_comma.push(text.trim_end().ends_with(','));
    }
    let name_max = rows
        .iter()
        .map(|(_, n, _)| display_width(n, f.cfg.tab_width as usize))
        .max()
        .unwrap_or(0);
    // 参数 prefix 最大宽度（带宽度类型时 name 对齐到 max_prefix + 1）
    let max_prefix_w = rows
        .iter()
        .map(|(p, _, _)| display_width(p, f.cfg.tab_width as usize))
        .max()
        .unwrap_or(0);
    // 生成参数行 + 注释事件
    let mut events: Vec<(bool, String, usize, usize)> = Vec::new();
    let mut pi = 0usize;
    let mut pending: Option<(usize, usize, String)> = None;
    for item in &items {
        match item.kind() {
            "#" | "(" | ")" => {}
            "parameter_port_declaration" => {
                if let Some((s, e, line)) = pending.take() {
                    events.push((true, line, s, e));
                }
                let idx = pi;
                pi += 1;
                let is_last = idx + 1 == params.len();
                // name 起始列：无类型参数（`parameter NAME`）对齐到 max_prefix+1，带类型参数从 prefix+1
                let prefix = &rows[idx].0;
                let is_typed = prefix.trim() != "parameter";
                // 带宽度类型（含 `[`）或无类型参数对齐到 max_prefix+1，带类型参数从 prefix+1
                let name_col = if prefix.contains('[') || !is_typed {
                    max_prefix_w + 1
                } else {
                    display_width(prefix, f.cfg.tab_width as usize) + 1
                };
                let mut line = fmt_parameter_port_line(
                    f,
                    prefix,
                    &rows[idx].1,
                    &rows[idx].2,
                    name_col,
                    name_max,
                    is_last,
                    has_comma[idx],
                );
                // 保留原文逗号后的行内尾随空格（仅纯空格，不含注释）
                if !is_last {
                    let next = params[idx + 1];
                    let ws = f.ws(item.byte_range().end, next.byte_range().start);
                    let trailing = f.trailing_inline(ws).trim_start_matches(',');
                    let cut = trailing.find("//").unwrap_or(trailing.len());
                    let trailing = &trailing[..cut];
                    if !trailing.is_empty() {
                        line.push_str(trailing);
                    }
                }
                pending = Some((item.byte_range().start, item.byte_range().end, line));
            }
            _ if item.is_named() && item.kind().ends_with("comment") => {
                let cmt_start = item.byte_range().start;
                if let Some((s, e, line)) = pending.take() {
                    let ws = f.ws(e, cmt_start);
                    if !ws.contains('\n') {
                        let line2 =
                            pad_comment(f, &line, item.text(), f.cfg.indent_width as usize, 0);
                        events.push((true, line2, s, item.byte_range().end));
                    } else {
                        events.push((true, line, s, e));
                        events.push((
                            false,
                            f.comment_text(*item),
                            cmt_start,
                            item.byte_range().end,
                        ));
                    }
                } else {
                    events.push((
                        false,
                        f.comment_text(*item),
                        cmt_start,
                        item.byte_range().end,
                    ));
                }
            }
            _ => {}
        }
    }
    if let Some((s, e, line)) = pending {
        events.push((true, line, s, e));
    }
    // 输出（参数行之间强制换行）
    let mut first = true;
    let mut prev_end: Option<usize> = None;
    for (is_param, text, start, end) in events {
        if !first {
            let ws = f.ws(prev_end.unwrap(), start);
            let blank_before = count_blank_lines(ws) > 0;
            if blank_before {
                docs.push(Doc::BlankLines(count_blank_lines(ws)));
            } else if has_newline(ws) || is_param {
                docs.push(Doc::Newline);
            } else {
                docs.push(Doc::Space);
            }
        }
        docs.push(Doc::text(text));
        prev_end = Some(end);
        first = false;
    }
    docs.push(Doc::Dedent);
    docs.push(Doc::Newline);
    docs.push(Doc::text(")"));
    Doc::concat(docs)
}

/// 参数行的列：参数名、`=`、默认值。
pub(super) fn parameter_columns(f: &Formatter<'_>, node: CstNode<'_>) -> (String, Vec<String>) {
    // 返回 (前缀如 "parameter" 或 "parameter int", [name, value])
    let items: Vec<CstNode<'_>> = node.children();
    let mut keyword = String::new();
    let mut type_str = String::new();
    let mut name = String::new();
    let mut value = String::new();
    // 处理 parameter_port_declaration -> parameter_declaration 或直接的 local/parameter 声明
    for c in items {
        match c.kind() {
            "parameter" | "localparam" => keyword = c.text().to_string(),
            "data_type" | "data_type_or_implicit" | "integer_vector_type" | "integer_atom_type" => {
                type_str = c.text().to_string();
            }
            "list_of_param_assignments" => {
                if let Some(p) = c.find_named_child("param_assignment") {
                    let pc: Vec<CstNode<'_>> = p.children();
                    for t in pc {
                        if t.kind() == "simple_identifier" {
                            name = t.text().to_string();
                        } else if t.kind() == "constant_param_expression" {
                            value = t.text().to_string();
                        }
                    }
                }
            }
            "parameter_declaration" | "local_parameter_declaration" => {
                for s in c.children() {
                    match s.kind() {
                        "parameter" | "localparam" => keyword = s.text().to_string(),
                        "data_type"
                        | "data_type_or_implicit"
                        | "integer_vector_type"
                        | "integer_atom_type" => type_str = s.text().to_string(),
                        "list_of_param_assignments" => {
                            if let Some(p) = s.find_named_child("param_assignment") {
                                for t in p.children() {
                                    if t.kind() == "simple_identifier" {
                                        name = t.text().to_string();
                                    } else if t.kind() == "constant_param_expression" {
                                        value = t.text().to_string();
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let mut prefix = keyword;
    if !type_str.is_empty() {
        // `parameter[3:0]`（无类型名直接带维度）不加空格
        if !type_str.starts_with('[') {
            prefix.push(' ');
        }
        prefix.push_str(&type_str);
    }
    let _ = f;
    (prefix, vec![name, value])
}

/// 参数端口列表的每条参数行。
#[allow(clippy::too_many_arguments)]
fn fmt_parameter_port_line(
    f: &Formatter<'_>,
    prefix: &str,
    name: &str,
    value: &str,
    name_col: usize,
    name_max: usize,
    is_last: bool,
    trailing_space: bool,
) -> String {
    let mut line = String::new();
    line.push_str(prefix);
    // prefix 后补空格到 name 起始列
    let cur = display_width(prefix, f.cfg.tab_width as usize);
    if cur < name_col {
        for _ in 0..(name_col - cur) {
            line.push(' ');
        }
    }
    let mut cell = name.to_string();
    cell = pad_to(&cell, name_max + 1, f.cfg.tab_width as usize);
    line.push_str(&cell);
    line.push('=');
    line.push(' ');
    line.push_str(value);
    if !is_last || trailing_space {
        line.push(',');
    }
    line
}
