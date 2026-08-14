//! 声明与赋值布局。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::formatter::alignment::pad_to;
use crate::formatter::expressions::fmt_default;
use crate::formatter::tokens::display_width;
use crate::parser::CstNode;

/// data_declaration / net_declaration。
pub fn fmt_data_declaration(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    // typedef 等类型声明原样保留
    if node
        .named_children()
        .iter()
        .any(|c| c.kind() == "type_declaration")
    {
        return Doc::text(node.text().trim_end().to_string());
    }
    // 模块体内由对齐段处理；此处用于非对齐上下文（如 seq_block 内）
    fmt_default(f, node)
}

/// parameter / localparam 声明（支持多行参数对齐）。
pub fn fmt_parameter_declaration(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let children: Vec<CstNode<'_>> = node.children();
    let mut keyword = String::new();
    let mut type_str: Option<String> = None;
    let mut params: Vec<(String, String)> = Vec::new();
    let mut param_spans: Vec<(usize, usize)> = Vec::new(); // 每个参数的 (end, 下一参数 start)

    for c in &children {
        match c.kind() {
            "localparam" | "parameter" => keyword = c.text().to_string(),
            "data_type" | "data_type_or_implicit" | "integer_vector_type" | "integer_atom_type" => {
                type_str = Some(c.text().to_string());
            }
            "list_of_param_assignments" => {
                let assignments: Vec<CstNode<'_>> = c
                    .named_children()
                    .into_iter()
                    .filter(|p| p.kind() == "param_assignment")
                    .collect();
                for (pi, p) in assignments.iter().enumerate() {
                    let mut name = String::new();
                    let mut value = String::new();
                    for t in p.children() {
                        if t.kind() == "simple_identifier" {
                            name = t.text().to_string();
                        } else if t.kind() == "constant_param_expression" {
                            value = t.text().to_string();
                        }
                    }
                    params.push((name, value));
                    if pi + 1 < assignments.len() {
                        let e = p.byte_range().end;
                        let s = assignments[pi + 1].byte_range().start;
                        param_spans.push((e, s));
                    }
                }
            }
            _ => {}
        }
    }
    if params.is_empty() {
        return fmt_default(f, node);
    }
    let name_max = params
        .iter()
        .map(|(n, _)| display_width(n, f.cfg.tab_width as usize))
        .max()
        .unwrap_or(0);
    let mut docs: Vec<Doc> = Vec::new();
    docs.push(Doc::text(keyword.clone()));
    if let Some(t) = &type_str {
        docs.push(Doc::Space);
        docs.push(Doc::text(t.clone()));
    }
    docs.push(Doc::Space);
    let name_col = display_width(&keyword, f.cfg.tab_width as usize) + 1;
    for (i, (name, value)) in params.iter().enumerate() {
        if i > 0 {
            docs.push(Doc::Newline);
            // 对齐到 name 列
            let mut pad = String::new();
            for _ in 0..name_col {
                pad.push(' ');
            }
            docs.push(Doc::text(pad));
        }
        let mut cell = name.to_string();
        cell = pad_to(&cell, name_max + 1, f.cfg.tab_width as usize);
        cell.push('=');
        docs.push(Doc::text(cell));
        docs.push(Doc::Space);
        docs.push(Doc::text(value.clone()));
        let is_last = i + 1 == params.len();
        if !is_last {
            docs.push(Doc::text(","));
            // 保留原文逗号后的行内尾随空格（如 `3'b000, `）
            if let Some((e, s)) = param_spans.get(i) {
                let ws = f.ws(*e, *s);
                let trailing = f.trailing_inline(ws);
                let trailing = trailing.trim_start_matches(',');
                if !trailing.is_empty() {
                    docs.push(Doc::text(trailing.to_string()));
                }
            }
        }
    }
    docs.push(Doc::text(";"));
    Doc::concat(docs)
}

/// continuous_assign（`assign` 语句）。
pub fn fmt_continuous_assign(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    fmt_default(f, node)
}

/// typedef 声明：保持原文。
pub fn fmt_typedef(_f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    Doc::text(node.text().trim_end().to_string())
}
