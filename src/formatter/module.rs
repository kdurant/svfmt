//! 模块布局：module 声明、参数列表、端口列表、模块体。

use crate::document::{render, Doc, RenderOptions};
use crate::formatter::alignment::{align_rows, pad_to};
use crate::formatter::expressions::{ExprCtx, fmt_expr};
use crate::formatter::tokens::{display_width, has_newline, leaf_tokens};
use crate::formatter::{Formatter, count_blank_lines};
use crate::parser::CstNode;

/// module_declaration。
pub fn fmt_module_declaration(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();
    let mut header: Option<CstNode<'_>> = None;
    let mut endmodule: Option<CstNode<'_>> = None;
    let mut body: Vec<CstNode<'_>> = Vec::new();
    for c in &items {
        if c.kind() == "module_ansi_header" || c.kind() == "module_nonansi_header" {
            header = Some(*c);
        } else if c.kind() == "endmodule" {
            endmodule = Some(*c);
        } else {
            body.push(*c);
        }
    }
    if let Some(h) = header {
        docs.push(fmt_module_header(f, h));
    }
    // 模块体
    docs.push(Doc::Newline);
    if let (Some(h), Some(first_body)) = (header, body.first()) {
        let ws = f.ws(h.byte_range().end, first_body.byte_range().start);
        let blanks = count_blank_lines(ws);
        if blanks > 0 {
            docs.push(Doc::BlankLines(blanks));
        }
    }
    if f.cfg.indent_module_contents {
        docs.push(Doc::Indent);
    }
    append_body_items(f, &mut docs, &body, true);
    if f.cfg.indent_module_contents {
        docs.push(Doc::Dedent);
    }
    if let Some(e) = endmodule {
        // endmodule 前保留空行
        let blank_before = body
            .last()
            .map(|b| {
                let ws = f.ws(b.byte_range().end, e.byte_range().start);
                count_blank_lines(ws) > 0
            })
            .unwrap_or(false);
        if blank_before {
            let blanks = body
                .last()
                .map(|b| count_blank_lines(f.ws(b.byte_range().end, e.byte_range().start)))
                .unwrap_or(0);
            docs.push(Doc::BlankLines(blanks));
        } else {
            docs.push(Doc::Newline);
        }
        docs.push(Doc::text("endmodule"));
        let _ = e;
    }
    Doc::concat(docs)
}

/// 模块体 items 顺序输出，保留空行；连续声明/赋值对齐。
fn append_body_items(f: &Formatter<'_>, docs: &mut Vec<Doc>, body: &[CstNode<'_>], _module: bool) {
    // 分成对齐段：段内为连续的"可对齐行"（data_declaration 或 continuous_assign），
    // 中间可夹注释；空行断段。
    let mut seg: Vec<CstNode<'_>> = Vec::new();
    let flush = |f: &Formatter<'_>, seg: &mut Vec<CstNode<'_>>, docs: &mut Vec<Doc>| {
        if seg.is_empty() {
            return;
        }
        emit_aligned_segment(f, seg, docs);
        seg.clear();
    };
    let mut prev_end: Option<usize> = None;
    for raw in body {
        // 解开 module_item 包装
        let mut cur = *raw;
        if cur.kind() == "module_item"
            && let Some(inner) = cur.named_child(0)
        {
            cur = inner;
        }
        let c = cur;
        let is_comment = c.is_named() && c.kind().ends_with("comment");
        let alignable = is_alignable(c);
        // 与前一个元素之间是否有空行 → 断段
        let blank_before = prev_end
            .map(|pe| count_blank_lines(f.ws(pe, c.byte_range().start)) > 0)
            .unwrap_or(false);
        if blank_before {
            flush(f, &mut seg, docs);
        }
        // 同类才能在同一对齐段（声明（data/net 同组）与 assign 分开对齐）
        if alignable && !seg.is_empty() && !blank_before {
            let last_kind = seg
                .iter()
                .rev()
                .find(|n| !(n.is_named() && n.kind().ends_with("comment")))
                .map(|n| n.kind())
                .unwrap_or("");
            let same_group = (last_kind == c.kind())
                || (is_decl(last_kind) && is_decl(c.kind()))
                || (decl_has_eq(f, *seg.last().unwrap()) && decl_has_eq(f, c));
            if !same_group {
                flush(f, &mut seg, docs);
            }
        }
        if !alignable && !is_comment {
            flush(f, &mut seg, docs);
        }
        // 处理与上一 item 的间隔（空行/换行）
        if !docs.is_empty() && !matches!(docs.last(), Some(Doc::Newline) | Some(Doc::BlankLines(_)))
        {
            let start = c.byte_range().start;
            let ws = f.ws(prev_end.unwrap(), start);
            if has_newline(ws) {
                let blanks = count_blank_lines(ws);
                if blanks > 0 {
                    docs.push(Doc::BlankLines(blanks));
                } else {
                    docs.push(Doc::Newline);
                }
            } else {
                docs.push(Doc::Space);
            }
        }
        if is_comment {
            // 空行后的注释：独立输出；同行注释：加入段作行尾注释
            if blank_before || seg.is_empty() {
                docs.push(f.fmt(c));
            } else {
                seg.push(c);
            }
        } else if alignable {
            seg.push(c);
        } else {
            docs.push(f.fmt(c));
        }
        prev_end = Some(c.byte_range().end);
    }
    flush(f, &mut seg, docs);
}

fn is_alignable(node: CstNode<'_>) -> bool {
    if node.kind() == "data_declaration" || node.kind() == "net_declaration" {
        // typedef 等类型声明不参与普通对齐（原样输出）
        if node
            .named_children()
            .iter()
            .any(|c| c.kind() == "type_declaration")
        {
            return false;
        }
        return true;
    }
    if node.kind() == "local_parameter_declaration" || node.kind() == "parameter_declaration" {
        // 多参数声明（`localparam A = 1, B = 2`）单独处理
        let multi = node.named_children().iter().any(|c| {
            if c.kind() == "list_of_param_assignments" {
                c.named_children()
                    .iter()
                    .filter(|p| p.kind() == "param_assignment")
                    .count()
                    > 1
            } else {
                false
            }
        });
        return !multi;
    }
    matches!(node.kind(), "continuous_assign")
}

fn is_decl(kind: &str) -> bool {
    matches!(
        kind,
        "data_declaration"
            | "net_declaration"
            | "local_parameter_declaration"
            | "parameter_declaration"
    )
}

/// 判断可对齐行是否含 `=`（赋值/初始化）。
fn decl_has_eq(f: &Formatter<'_>, node: CstNode<'_>) -> bool {
    if matches!(
        node.kind(),
        "continuous_assign" | "local_parameter_declaration" | "parameter_declaration"
    ) {
        return true;
    }
    let _ = f;
    node.text().contains('=')
}

/// 输出一个对齐段。
fn emit_aligned_segment(f: &Formatter<'_>, seg: &[CstNode<'_>], docs: &mut Vec<Doc>) {
    // 检测段内是否有赋值/初始化 `=`（localparam、assign、带初始化的声明）
    let has_eq = seg.iter().any(|n| {
        matches!(
            n.kind(),
            "continuous_assign" | "local_parameter_declaration" | "parameter_declaration"
        ) || (matches!(n.kind(), "data_declaration" | "net_declaration") && n.text().contains('='))
    });
    if has_eq {
        if std::env::var("SVDBG").is_ok() {
            eprintln!(
                "[eq-seg] kinds={:?}",
                seg.iter().map(|n| n.kind()).collect::<Vec<_>>()
            );
        }
        emit_eq_segment(f, seg, docs);
        return;
    }
    // 行内容：前缀 + 列文本
    let mut prefixes: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for node in seg {
        if node.kind() == "data_declaration" || node.kind() == "net_declaration" {
            prefixes.push(String::new());
            rows.push(declaration_columns(f, *node));
        } else if node.kind() == "continuous_assign" {
            prefixes.push("assign ".to_string());
            let cols = assign_columns(f, *node);
            rows.push(cols);
        } else if node.kind() == "local_parameter_declaration"
            || node.kind() == "parameter_declaration"
        {
            let (keyword, cols) = parameter_columns(f, *node);
            prefixes.push(format!("{keyword} "));
            rows.push(vec![cols[0].clone(), "=".to_string(), cols[1].clone()]);
        } else {
            // 注释等：不参与对齐，空列占位
            prefixes.push(String::new());
            rows.push(vec![]);
        }
    }
    // 对齐
    if std::env::var("SVDBG").is_ok() {
        eprintln!(
            "[seg] kinds={:?}",
            seg.iter().map(|s| s.kind()).collect::<Vec<_>>()
        );
        eprintln!("[seg] rows={:?}", rows);
    }
    let aligned = align_rows(&rows, f.cfg.tab_width as usize);
    // 计算块内最长行的 `;` 位置（注释对齐到 max(comment_column, max_semi+3)）
    let block_max_semi = seg
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            matches!(
                n.kind(),
                "continuous_assign"
                    | "data_declaration"
                    | "net_declaration"
                    | "local_parameter_declaration"
                    | "parameter_declaration"
            )
        })
        .map(|(i, _)| {
            let base = format!("{}{}", prefixes[i], aligned[i]);
            base.len()
        })
        .max()
        .unwrap_or(0);
    // 生成完整行序列
    let mut lines: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    let mut prev_end: Option<usize> = None;
    for (i, node) in seg.iter().enumerate() {
        let is_decl_or_assign = matches!(
            node.kind(),
            "continuous_assign"
                | "data_declaration"
                | "net_declaration"
                | "local_parameter_declaration"
                | "parameter_declaration"
        );
        if is_decl_or_assign {
            if let Some(l) = current.take() {
                lines.push(l);
            }
            let mut line = format!("{}{}", prefixes[i], aligned[i]);
            if !line.trim_end().ends_with(';') {
                line.push(';');
            }
            // 声明节点内部的行尾注释
            if let Some(comment) = trailing_comment(f, *node) {
                line = pad_comment(f, &line, &comment, 0, block_max_semi);
                lines.push(line);
                current = None;
            } else {
                current = Some(line);
            }
        } else {
            // 注释：若与前一行同行 → 行尾注释；否则独立行
            let ws = prev_end
                .map(|pe| f.ws(pe, node.byte_range().start))
                .unwrap_or("");
            if let Some(l) = current.take() {
                if !ws.contains('\n') {
                    lines.push(pad_comment(f, &l, node.text(), 0, block_max_semi));
                } else {
                    lines.push(l);
                    lines.push(node.text().to_string());
                }
            } else {
                lines.push(node.text().to_string());
            }
        }
        prev_end = Some(node.byte_range().end);
    }
    if let Some(l) = current {
        lines.push(l);
    }
    // 输出行序列，行间换行
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            docs.push(Doc::Newline);
        }
        docs.push(Doc::text(line.clone()));
    }
}

/// 提取节点行尾的注释文本。
fn trailing_comment(f: &Formatter<'_>, node: CstNode<'_>) -> Option<String> {
    let toks = leaf_tokens(node);
    for t in &toks {
        if t.is_comment {
            return Some(t.text.to_string());
        }
    }
    let _ = f;
    None
}

/// 供 statements 等模块使用的行尾注释对齐。
pub fn pad_comment_pub(
    f: &Formatter<'_>,
    line: &str,
    comment: &str,
    base_indent: usize,
    block_max_semi: usize,
) -> String {
    pad_comment(f, line, comment, base_indent, block_max_semi)
}

/// 行尾注释对齐：相对列 = max(comment_column - base_indent, block_max_semi + 3)。
fn pad_comment(
    f: &Formatter<'_>,
    line: &str,
    comment: &str,
    base_indent: usize,
    block_max_semi: usize,
) -> String {
    let target_rel = (f.cfg.comment_column as usize)
        .saturating_sub(base_indent)
        .max(block_max_semi + 3);
    let base = line.trim_end().to_string();
    let w = display_width(&base, f.cfg.tab_width as usize);
    let mut out = base;
    if w >= target_rel {
        // 超长：按 comment_indent 空格分隔
        for _ in 0..f.cfg.comment_indent {
            out.push(' ');
        }
    } else {
        out = pad_to(&out, target_rel, f.cfg.tab_width as usize);
    }
    out.push_str(comment);
    out
}

/// 声明行的列内容：类型+dim 组合、名字（+unpacked dim）。
fn declaration_columns(f: &Formatter<'_>, node: CstNode<'_>) -> Vec<String> {
    let _ = f;
    // data_declaration: 类型 + 名称
    let items: Vec<CstNode<'_>> = node.children();
    let mut type_part = String::new();
    let mut name_part = String::new();
    let mut after_type = false;
    for c in items {
        let kind = c.kind();
        if kind == ";" {
            break;
        }
        if kind == "data_type"
            || kind == "data_type_or_implicit"
            || kind == "net_type"
            || kind == "implicit_data_type"
            || kind == "integer_vector_type"
            || kind == "integer_atom_type"
            || kind == "signing"
            || kind == "packed_dimension"
            || kind == "integer_type"
            || kind == "simple_type"
            || kind == "type_reference"
            || kind == "variable_type"
        {
            if !type_part.is_empty() && !type_part.ends_with(' ') {
                type_part.push(' ');
            }
            type_part.push_str(c.text());
        } else if kind == "list_of_variable_identifiers"
            || kind == "list_of_net_identifiers"
            || kind == "variable_identifier_list"
            || kind == "net_identifier_list"
            || kind == "list_of_variable_decl_assignments"
            || kind == "list_of_net_decl_assignments"
        {
            // 名称（含 unpacked dim 与初始化），压缩 ` [  ` 为 `[`
            let raw = c.text();
            name_part = raw.replace(" [", "[");
            after_type = true;
        } else if after_type {
            break;
        }
    }
    if type_part.is_empty() {
        type_part = "".to_string();
    }
    let type_part = type_part.split_whitespace().collect::<Vec<_>>().join(" ");
    vec![type_part, name_part]
}

/// assign 行的列内容：LHS、`=`、RHS。
fn assign_columns(f: &Formatter<'_>, node: CstNode<'_>) -> Vec<String> {
    let mut lhs = String::new();
    let mut rhs_text = String::new();
    for c in node.children() {
        if c.kind() == "list_of_net_assignments" || c.kind() == "list_of_variable_assignments" {
            for sub in c.children() {
                if sub.is_named() {
                    let cols = assignment_columns(f, sub);
                    lhs = cols.0;
                    rhs_text = cols.1;
                }
            }
        }
    }
    vec![lhs, "=".to_string(), rhs_text]
}

/// net_assignment / variable_assignment 的 LHS 与 RHS（RHS 用表达式格式化规范）。
fn assignment_columns(f: &Formatter<'_>, node: CstNode<'_>) -> (String, String) {
    let mut lhs = String::new();
    let mut rhs_parts: Vec<CstNode<'_>> = Vec::new();
    for c in node.children() {
        if c.kind() == "=" || c.kind() == "<=" {
            continue;
        }
        if lhs.is_empty() {
            lhs = c.text().to_string();
        } else {
            rhs_parts.push(c);
        }
    }
    let rhs_text = rhs_parts
        .iter()
        .map(|r| {
            if r.kind() == "ERROR" {
                // 宏前反引号：输出 ` + 空格（保持 ` PROJECT` 形态）
                format!("{} ", r.text())
            } else {
                let doc = fmt_expr(f, *r, &ExprCtx::default());
                render(&doc, &RenderOptions::from(f.cfg))
            }
        })
        .collect::<String>();
    (lhs, rhs_text)
}

/// module_ansi_header / module_nonansi_header。
pub fn fmt_module_header(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();
    let mut i = 0;
    // module + name
    while i < items.len() {
        let c = items[i];
        if c.kind() == "module_keyword" {
            docs.push(Doc::text("module"));
            i += 1;
        } else if c.kind() == "simple_identifier" {
            docs.push(Doc::Space);
            docs.push(Doc::text(c.text()));
            i += 1;
        } else {
            break;
        }
    }
    // parameter_port_list
    let mut port_paren_break = false;
    let mut seen_semi = false;
    while i < items.len() {
        let c = items[i];
        match c.kind() {
            "parameter_port_list" => {
                let pl = fmt_parameter_port_list(f, c);
                docs.push(Doc::Space);
                docs.push(pl);
                i += 1;
            }
            "list_of_port_declarations" => {
                // 换行 + (
                docs.push(Doc::Newline);
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
                // 非 ANSI：空则 `()`，否则 `(...)`
                let text = c.text();
                docs.push(Doc::text(text.trim_end().to_string()));
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
pub fn fmt_parameter_port_list(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
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
                let mut line = fmt_parameter_port_line(
                    f,
                    &rows[idx].0,
                    &rows[idx].1,
                    &rows[idx].2,
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
                            item.text().to_string(),
                            cmt_start,
                            item.byte_range().end,
                        ));
                    }
                } else {
                    events.push((
                        false,
                        item.text().to_string(),
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
    // 输出
    let mut first = true;
    let mut prev_end: Option<usize> = None;
    for (_, text, start, end) in events {
        if !first {
            let ws = f.ws(prev_end.unwrap(), start);
            if has_newline(ws) {
                let blanks = count_blank_lines(ws);
                if blanks > 0 {
                    docs.push(Doc::BlankLines(blanks));
                } else {
                    docs.push(Doc::Newline);
                }
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
fn parameter_columns(f: &Formatter<'_>, node: CstNode<'_>) -> (String, Vec<String>) {
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
fn fmt_parameter_port_line(
    f: &Formatter<'_>,
    prefix: &str,
    name: &str,
    value: &str,
    name_max: usize,
    is_last: bool,
    trailing_space: bool,
) -> String {
    let mut line = String::new();
    line.push_str(prefix);
    line.push(' ');
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

/// 端口列表（list_of_port_declarations）：每行一个端口，列对齐，处理行尾注释。
fn fmt_port_declarations(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let items: Vec<CstNode<'_>> = node.children();
    let ports: Vec<CstNode<'_>> = items
        .iter()
        .filter(|c| c.kind() == "ansi_port_declaration")
        .copied()
        .collect();
    // 每行：方向、类型+dim、名字
    let mut parsed: Vec<(bool, String, String, String)> = Vec::new(); // (is_interface, dir, type_dim, name)
    for p in &ports {
        parsed.push(port_columns(f, *p));
    }
    // 端口名列：普通 = 方向7 + 类型宽 + 1；接口 = 类型宽 + 1
    let dir_col = 7usize;
    let mut name_col = 0usize;
    for (is_if, _, type_dim, _) in &parsed {
        let total = if *is_if {
            display_width(type_dim, f.cfg.tab_width as usize) + 1
        } else {
            dir_col + display_width(type_dim, f.cfg.tab_width as usize) + 1
        };
        if total > name_col {
            name_col = total;
        }
    }
    let mut lines: Vec<String> = Vec::new();
    for (is_if, dir, type_dim, name) in &parsed {
        let mut line = String::new();
        if *is_if {
            let t = pad_to(type_dim, name_col, f.cfg.tab_width as usize);
            line.push_str(&t);
        } else {
            let d = pad_to(dir, dir_col, f.cfg.tab_width as usize);
            line.push_str(&d);
            let t = pad_to(type_dim, name_col - dir_col, f.cfg.tab_width as usize);
            line.push_str(&t);
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
                            item.text().to_string(),
                            cmt_start,
                            item.byte_range().end,
                        ));
                    }
                } else {
                    events.push((
                        false,
                        item.text().to_string(),
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
    for (_, text, start, end) in events {
        if !first {
            let ws = f.ws(prev_end.unwrap(), start);
            if has_newline(ws) {
                let blanks = count_blank_lines(ws);
                if blanks > 0 {
                    docs.push(Doc::BlankLines(blanks));
                } else {
                    docs.push(Doc::Newline);
                }
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

/// 端口行列：方向、类型+dim、名字(+unpacked)。
fn port_columns(f: &Formatter<'_>, node: CstNode<'_>) -> (bool, String, String, String) {
    let items: Vec<CstNode<'_>> = node.children();
    let mut direction = String::new();
    let mut type_dim = String::new();
    let mut name = String::new();
    let mut unpacked = String::new();
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
                for h in header {
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
    let _ = f;
    (is_interface, direction, type_dim, name_full)
}

/// 供 fmt 调用的端口列表入口。
pub fn fmt_port_list(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
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
pub fn fmt_cond_expression(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    Doc::concat(vec![
        Doc::text("("),
        fmt_expr(f, node, &ExprCtx::default()),
        Doc::text(")"),
    ])
}

/// `=` 对齐段：localparam/assign/带初始化声明按赋值对齐。
fn emit_eq_segment(f: &Formatter<'_>, seg: &[CstNode<'_>], docs: &mut Vec<Doc>) {
    let mut contents: Vec<String> = Vec::new();
    let mut values: Vec<String> = Vec::new();
    let mut is_comment: Vec<bool> = Vec::new();
    let mut max_content = 0usize;
    for node in seg {
        if node.is_named() && node.kind().ends_with("comment") {
            contents.push(String::new());
            values.push(String::new());
            is_comment.push(true);
            continue;
        }
        let (content, value) = eq_columns(f, *node);
        if std::env::var("SVDBG").is_ok() {
            eprintln!(
                "[eq-cols] kind={} content={:?} value={:?}",
                node.kind(),
                content,
                value
            );
        }
        let w = display_width(&content, f.cfg.tab_width as usize);
        if w > max_content {
            max_content = w;
        }
        contents.push(content);
        values.push(value);
        is_comment.push(false);
    }
    let op_col = max_content + 1;
    // 生成行
    let mut lines: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    let mut prev_end: Option<usize> = None;
    for (i, node) in seg.iter().enumerate() {
        if is_comment[i] {
            let ws = prev_end
                .map(|pe| f.ws(pe, node.byte_range().start))
                .unwrap_or("");
            if let Some(l) = current.take() {
                if !ws.contains('\n') {
                    lines.push(pad_comment(f, &l, node.text(), 0, max_content));
                } else {
                    lines.push(l);
                    lines.push(node.text().to_string());
                }
            } else {
                lines.push(node.text().to_string());
            }
        } else {
            if let Some(l) = current.take() {
                lines.push(l);
            }
            let mut line = pad_to(&contents[i], op_col, f.cfg.tab_width as usize);
            line.push('=');
            if !values[i].is_empty() {
                line.push(' ');
                line.push_str(&values[i]);
            }
            line.push(';');
            current = Some(line);
        }
        prev_end = Some(node.byte_range().end);
    }
    if let Some(l) = current {
        lines.push(l);
    }
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            docs.push(Doc::Newline);
        }
        docs.push(Doc::text(line.clone()));
    }
}

/// 提取 `=` 对齐的行内容与值。
fn eq_columns(f: &Formatter<'_>, node: CstNode<'_>) -> (String, String) {
    match node.kind() {
        "data_declaration" | "net_declaration" => {
            let cols = declaration_columns(f, node);
            let mut name = cols[1].clone();
            let mut value = String::new();
            if let Some(pos) = name.find('=') {
                value = name[pos + 1..].trim().to_string();
                name = name[..pos].trim().to_string();
            }
            let content = format!("{} {}", cols[0], name).trim().to_string();
            (content, value)
        }
        "local_parameter_declaration" | "parameter_declaration" => {
            let (prefix, cols) = parameter_columns(f, node);
            (
                format!("{prefix} {}", cols[0]).trim().to_string(),
                cols[1].clone(),
            )
        }
        "continuous_assign" => {
            let cols = assign_columns(f, node);
            (format!("assign {}", cols[0]), cols[2].clone())
        }
        _ => (node.text().to_string(), String::new()),
    }
}
