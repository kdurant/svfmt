//! 端口列表布局。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::formatter::alignment::pad_to;
use crate::formatter::count_blank_lines;
use crate::formatter::expressions::{ExprCtx, fmt_expr};
use crate::formatter::tokens::{display_width, has_newline};
use crate::parser::CstNode;

use super::pad_comment;

/// 端口列表（list_of_port_declarations）：每行一个端口，列对齐，处理行尾注释。
pub(crate) fn fmt_port_declarations(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let items: Vec<CstNode<'_>> = node.children();
    let ports: Vec<CstNode<'_>> = items
        .iter()
        .filter(|c| c.kind() == "ansi_port_declaration")
        .copied()
        .collect();
    // 每行：方向、类型关键字、宽度、名字（接口无方向，类型即接口名）
    let mut parsed: Vec<(bool, String, String, String, String)> = Vec::new(); // (is_interface, dir, type_kw, width, name)
    let tw = f.cfg.tab_width as usize;
    for p in &ports {
        let (is_if, dir, type_dim, name) = port_columns(f, *p);
        let (type_kw, width) = split_type_width(&type_dim);
        parsed.push((is_if, dir, type_kw, width, name));
    }
    // 宽度列：存在“类型+宽度”端口时，无类型端口的宽度对齐到 类型列 + 最长类型关键字 + 1
    let dir_col = 7usize;
    let mut width_col = dir_col;
    for (is_if, _, type_kw, width, _) in &parsed {
        if !is_if && !type_kw.is_empty() && !width.is_empty() {
            let w = dir_col + display_width(type_kw, tw) + 1;
            if w > width_col {
                width_col = w;
            }
        }
    }
    // 端口名列：普通 = 方向7 + 类型区宽 + 1；接口 = 类型宽 + 1
    let mut name_col = 0usize;
    for (is_if, _, type_kw, width, _) in &parsed {
        let total = if *is_if {
            display_width(type_kw, tw) + 1
        } else if !type_kw.is_empty() {
            dir_col + display_width(&combine_type(type_kw, width), tw) + 1
        } else {
            width_col + display_width(width, tw) + 1
        };
        if total > name_col {
            name_col = total;
        }
    }
    let mut lines: Vec<String> = Vec::new();
    for (is_if, dir, type_kw, width, name) in &parsed {
        let mut line = String::new();
        if *is_if {
            let t = pad_to(type_kw, name_col, tw);
            line.push_str(&t);
        } else if f.cfg.module.port_alignment {
            let d = pad_to(dir, dir_col, tw);
            line.push_str(&d);
            if type_kw.is_empty() {
                // 无类型关键字：宽度对齐到宽度列
                let mut tmp = String::new();
                for _ in 0..width_col.saturating_sub(dir_col) {
                    tmp.push(' ');
                }
                tmp.push_str(width);
                line.push_str(&pad_to(&tmp, name_col - dir_col, tw));
            } else {
                let t = pad_to(&combine_type(type_kw, width), name_col - dir_col, tw);
                line.push_str(&t);
            }
        } else {
            // 关闭对齐：单空格分隔
            let combined = combine_type(type_kw, width);
            if !dir.is_empty() {
                line.push_str(dir);
                line.push(' ');
            }
            if !combined.is_empty() {
                line.push_str(&combined);
                line.push(' ');
            }
        }
        line.push_str(name);
        lines.push(line);
    }

    // 按顺序生成输出项：端口行 / 独立注释
    let mut events: Vec<(bool, String, usize, usize)> = Vec::new();
    let mut pi = 0usize;
    let mut pending: Option<(usize, usize, String)> = None;
    let has_comma_after: Vec<bool> = ports
        .iter()
        .map(|p| p.text().trim_end().ends_with(','))
        .collect();
    for item in &items {
        match item.kind() {
            "(" | ")" => {}
            "ansi_port_declaration" => {
                if let Some((s, e, mut line)) = pending.take() {
                    // 保留原文逗号后的行内尾随空格（仅纯空格，不含注释）
                    let ws = f.ws(e, item.byte_range().start);
                    let trailing = f.trailing_inline(ws).trim_start_matches(',');
                    if !trailing.contains("//") && !trailing.is_empty() {
                        line.push_str(trailing);
                    }
                    events.push((true, line, s, e));
                }
                let idx = pi;
                pi += 1;
                let mut line = lines[idx].clone();
                if idx + 1 < ports.len() || has_comma_after[idx] {
                    line.push(',');
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

    // 输出事件序列
    let mut docs: Vec<Doc> = Vec::new();
    let mut first = true;
    let mut prev_end: Option<usize> = None;
    for (is_port_line, text, start, end) in events {
        if !first {
            let ws = f.ws(prev_end.unwrap(), start);
            let blank_before = count_blank_lines(ws) > 0;
            // newline_per_port：端口行之间强制换行
            if blank_before {
                docs.push(Doc::BlankLines(count_blank_lines(ws)));
            } else if has_newline(ws) || (is_port_line && f.cfg.module.newline_per_port) {
                docs.push(Doc::Newline);
            } else {
                docs.push(Doc::Space);
            }
        }
        docs.push(Doc::text(text));
        prev_end = Some(end);
        first = false;
    }
    Doc::concat(docs)
}

/// 拆分类型与宽度：`reg [31:0]` → (`reg`, `[31:0]`)；`[7:0]` → (``, `[7:0]`)。
fn split_type_width(type_dim: &str) -> (String, String) {
    if let Some(pos) = type_dim.find('[') {
        let t = type_dim[..pos].trim_end().to_string();
        (t, type_dim[pos..].to_string())
    } else {
        (type_dim.to_string(), String::new())
    }
}

/// 合并类型关键字与宽度为展示用字符串。
fn combine_type(type_kw: &str, width: &str) -> String {
    if width.is_empty() {
        type_kw.to_string()
    } else if type_kw.is_empty() {
        width.to_string()
    } else {
        format!("{type_kw} {width}")
    }
}

/// 判断类型节点是否为"单个裸标识符"（如接口名 `if_gmii`），
/// 即 `data_type -> simple_identifier` 且无宽度。
fn is_bare_identifier_type(node: CstNode<'_>) -> bool {
    let t = match node.find_named_child("data_type") {
        Some(dt) => dt,
        None => node,
    };
    let named: Vec<CstNode<'_>> = t.named_children();
    named.len() == 1 && named[0].kind() == "simple_identifier"
}

/// 端口行列：方向、类型+dim、名字(+unpacked)。
fn port_columns(f: &Formatter<'_>, node: CstNode<'_>) -> (bool, String, String, String) {
    let items: Vec<CstNode<'_>> = node.children();
    let mut direction = String::new();
    let mut type_dim = String::new();
    let mut name = String::new();
    let mut unpacked = String::new();
    let mut default_val = String::new();
    let mut is_interface = false;
    for c in items {
        match c.kind() {
            "net_port_header" | "variable_port_header" | "interface_port_header" => {
                if c.kind() == "interface_port_header" {
                    is_interface = true;
                    // 接口类型直接使用原始文本（如 `if_axi_stream.slave`）
                    if type_dim.is_empty() {
                        type_dim = c.text().to_string();
                    }
                    continue;
                }
                let header: Vec<CstNode<'_>> = c.children();
                let mut dir = String::new();
                let mut rest = String::new();
                let mut seen_dir = false;
                for h in &header {
                    if h.kind() == "port_direction" {
                        dir = h.text().to_string();
                        seen_dir = true;
                    } else {
                        if !rest.is_empty() {
                            rest.push(' ');
                        }
                        rest.push_str(h.text());
                    }
                }
                if seen_dir {
                    direction = dir;
                } else if c.kind() == "variable_port_header"
                    && header
                        .iter()
                        .all(|h| h.kind() != "port_direction" && is_bare_identifier_type(*h))
                {
                    // 无方向的变量端口头且类型为单个裸标识符 → 接口端口
                    // （tree-sitter 对 `if_gmii gmii_i` 无 modport 的接口端口
                    //   会解析为 variable_port_header，这里恢复为接口处理）
                    is_interface = true;
                }
                if type_dim.is_empty() {
                    type_dim = rest;
                }
            }
            "port_direction" => {
                if direction.is_empty() {
                    direction = c.text().to_string();
                }
            }
            "net_port_type" | "variable_port_type" | "data_type" | "data_type_or_implicit" => {
                if type_dim.is_empty() {
                    type_dim = c.text().to_string();
                }
            }
            "packed_dimension" => {
                if !type_dim.is_empty() {
                    type_dim.push(' ');
                }
                type_dim.push_str(c.text());
            }
            "unpacked_dimension" => {
                unpacked.push_str(c.text());
            }
            "=" => {
                default_val.push('=');
            }
            "constant_expression" => {
                if !default_val.is_empty() {
                    default_val.push(' ');
                }
                // 归一化内部空白（如 `data_out = 32'hFF`）
                default_val.push_str(&c.text().split_whitespace().collect::<Vec<_>>().join(" "));
            }
            _ => {
                if let Some(n) = c.child_by_field_name("port_name") {
                    name = n.text().to_string();
                } else if c.kind() == "simple_identifier" && name.is_empty() {
                    name = c.text().to_string();
                }
            }
        }
    }
    let mut name_full = name;
    name_full.push_str(&unpacked);
    if !default_val.is_empty() {
        name_full.push(' ');
        name_full.push_str(&default_val);
    }
    let _ = f;
    (is_interface, direction, type_dim, name_full)
}

/// 供 fmt 调用的端口列表入口。
pub(crate) fn fmt_port_list(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    if node.kind() == "list_of_ports" {
        let text = node.text();
        if text.trim() == "()" {
            return Doc::text("()");
        }
        return Doc::text(text.to_string());
    }
    fmt_port_declarations(f, node)
}

/// 供 fmt_expression 使用的 cond 辅助。
pub(crate) fn fmt_cond_expression(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    Doc::concat(vec![
        Doc::text("("),
        fmt_expr(f, node, &ExprCtx::default()),
        Doc::text(")"),
    ])
}
