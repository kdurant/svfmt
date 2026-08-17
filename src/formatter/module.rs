//! 模块布局：module 声明、参数列表、端口列表、模块体。
//!
//! 按职责拆分为子模块：`params`（模块头/参数列表）、`ports`（端口列表）。

mod params;
mod ports;

use crate::document::{Doc, render_inline};
use crate::formatter::alignment::{align_rows, pad_to};
use crate::formatter::expressions::{ExprCtx, fmt_expr};
use crate::formatter::tokens::{display_width, has_newline, leaf_tokens};
use crate::formatter::{Formatter, count_blank_lines};
use crate::parser::CstNode;
use params::parameter_columns;

// 子模块对外接口 re-export（dispatch 与跨模块引用统一经此路径）
pub(crate) use params::{fmt_module_header, fmt_parameter_port_list};
pub(crate) use ports::{fmt_cond_expression, fmt_port_list};

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
    let mut prev_is_proc = false;
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
        let is_proc = is_procedural_item(c);
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
        if !docs.is_empty()
            && !matches!(docs.last(), Some(Doc::Newline) | Some(Doc::BlankLines(_)))
            && let Some(pe) = prev_end
        {
            let start = c.byte_range().start;
            let ws = f.ws(pe, start);
            if has_newline(ws) {
                let blanks = count_blank_lines(ws);
                if blanks > 0 {
                    docs.push(Doc::BlankLines(blanks));
                } else if f.cfg.blank_line_between_procedures && is_proc && prev_is_proc {
                    docs.push(Doc::BlankLines(1));
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
        prev_is_proc = is_proc;
    }
    flush(f, &mut seg, docs);
}

/// 是否为过程块（always/initial/final/always_comb 等）。
fn is_procedural_item(node: CstNode<'_>) -> bool {
    let kind = node.kind();
    kind.starts_with("always")
        || kind.starts_with("initial_")
        || kind.starts_with("final_")
        || kind == "procedural_construct"
        || kind.ends_with("_construct")
}

fn is_alignable(node: CstNode<'_>) -> bool {
    if node.kind() == "data_declaration" || node.kind() == "net_declaration" {
        // 含语法错误（ERROR）的声明：保留原文
        if node.subtree_has_error() {
            return false;
        }
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
    if node.kind() == "module_instantiation" {
        // 仅单行实例化参与对齐（多行参数列表实例化保持原样，避免其内部
        // 行尾注释被 pad_comment 重排，见 examples/packet.sv 的 xpm_fifo_axis）
        return !node.text().contains('\n');
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
    // 实例化语句的参数赋值 `#(.W(8))` 不是赋值对齐标志，不与带 `=` 的声明混组
    if node.kind() == "module_instantiation" {
        return false;
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
        if f.svdbg() {
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
        } else if node.kind() == "module_instantiation" {
            // 实例化语句：整行作为单列（不参与列对齐），仅用于行尾注释对齐
            prefixes.push(String::new());
            rows.push(vec![render_inline(&f.fmt(*node), f.cfg)]);
        } else {
            // 注释等：不参与对齐，空列占位
            prefixes.push(String::new());
            rows.push(vec![]);
        }
    }
    // 对齐
    if f.svdbg() {
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
                    | "module_instantiation"
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
    let mut current_is_inst = false;
    let mut prev_end: Option<usize> = None;
    for (i, node) in seg.iter().enumerate() {
        let is_decl_or_assign = matches!(
            node.kind(),
            "continuous_assign"
                | "data_declaration"
                | "net_declaration"
                | "local_parameter_declaration"
                | "parameter_declaration"
                | "module_instantiation"
        );
        if is_decl_or_assign {
            if let Some(l) = current.take() {
                lines.push(l);
            }
            let mut line = format!("{}{}", prefixes[i], aligned[i]);
            if !line.trim_end().ends_with(';') {
                line.push(';');
            }
            current_is_inst = node.kind() == "module_instantiation";
            // 声明节点内部的行尾注释
            if let Some(comment) = trailing_comment(f, *node) {
                line = pad_comment(f, &line, &comment, 0, block_max_semi);
                lines.push(line);
                current = None;
                current_is_inst = false;
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
                    if current_is_inst {
                        lines.push(pad_inst_comment(f, &l, node.text(), block_max_semi));
                    } else {
                        lines.push(pad_comment(f, &l, node.text(), 0, block_max_semi));
                    }
                } else {
                    lines.push(l);
                    lines.push(f.comment_text(*node));
                }
            } else {
                lines.push(f.comment_text(*node));
            }
            current_is_inst = false;
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
    if !f.cfg.align_trailing_comments {
        // 关闭对齐：固定 comment_indent 空格
        for _ in 0..f.cfg.comment_indent {
            out.push(' ');
        }
        out.push_str(comment);
        return out;
    }
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

/// 实例化行行尾注释对齐：对齐到最长行 + 1 空格（比声明/assign 的 +3 更紧凑）。
fn pad_inst_comment(f: &Formatter<'_>, line: &str, comment: &str, block_max_semi: usize) -> String {
    let mut out = line.trim_end().to_string();
    if !f.cfg.align_trailing_comments {
        // 关闭对齐：固定 comment_indent 空格
        for _ in 0..f.cfg.comment_indent {
            out.push(' ');
        }
        out.push_str(comment);
        return out;
    }
    let target = block_max_semi + 1;
    let w = display_width(&out, f.cfg.tab_width as usize);
    if w >= target {
        // 超长：按 comment_indent 空格分隔
        for _ in 0..f.cfg.comment_indent {
            out.push(' ');
        }
    } else {
        out = pad_to(&out, target, f.cfg.tab_width as usize);
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
            || kind == "simple_identifier"
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
    for c in node.children_iter() {
        if c.kind() == "list_of_net_assignments" || c.kind() == "list_of_variable_assignments" {
            for sub in c.children_iter() {
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
    for c in node.children_iter() {
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
                render_inline(&doc, f.cfg)
            }
        })
        .collect::<String>();
    (lhs, rhs_text)
}

/// `=` 对齐段：localparam/assign/带初始化声明按赋值对齐。
fn emit_eq_segment(f: &Formatter<'_>, seg: &[CstNode<'_>], docs: &mut Vec<Doc>) {
    // 每行：kind、type_dim（decl 的类型+宽度）、name、value
    let mut kinds: Vec<&'static str> = Vec::new();
    let mut type_dims: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut values: Vec<Option<String>> = Vec::new();
    let mut is_comment: Vec<bool> = Vec::new();
    let mut max_type = 0usize;
    for node in seg {
        if node.is_named() && node.kind().ends_with("comment") {
            kinds.push("");
            type_dims.push(String::new());
            names.push(String::new());
            values.push(None);
            is_comment.push(true);
            continue;
        }
        let (kind, type_dim, name, value) = eq_columns(f, *node);
        if kind == "decl" {
            let w = display_width(&type_dim, f.cfg.tab_width as usize);
            if w > max_type {
                max_type = w;
            }
        }
        kinds.push(kind);
        type_dims.push(type_dim);
        names.push(name);
        values.push(value);
        is_comment.push(false);
    }
    // 计算每行左列宽度，找最大
    let mut left_widths: Vec<usize> = Vec::new();
    let mut max_left = 0usize;
    for i in 0..seg.len() {
        if is_comment[i] || values[i].is_none() {
            left_widths.push(0);
            continue;
        }
        let w = match kinds[i] {
            "decl" => max_type + 1 + display_width(&names[i], f.cfg.tab_width as usize),
            "assign" => display_width(&type_dims[i], f.cfg.tab_width as usize),
            _ => {
                display_width(&type_dims[i], f.cfg.tab_width as usize)
                    + 1
                    + display_width(&names[i], f.cfg.tab_width as usize)
            }
        };
        if w > max_left {
            max_left = w;
        }
        left_widths.push(w);
    }
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
                    lines.push(pad_comment(f, &l, node.text(), 0, max_left));
                } else {
                    lines.push(l);
                    lines.push(f.comment_text(*node));
                }
            } else {
                lines.push(f.comment_text(*node));
            }
        } else {
            if let Some(l) = current.take() {
                lines.push(l);
            }
            // 左列
            let left = match kinds[i] {
                "decl" => {
                    let mut l = pad_to(&type_dims[i], max_type, f.cfg.tab_width as usize);
                    l.push(' ');
                    l.push_str(&names[i]);
                    l
                }
                _ => {
                    let mut l = type_dims[i].clone();
                    if !names[i].is_empty() {
                        l.push(' ');
                        l.push_str(&names[i]);
                    }
                    l
                }
            };
            let line = if let Some(v) = &values[i] {
                let mut l = pad_to(&left, max_left, f.cfg.tab_width as usize);
                l.push(' ');
                l.push('=');
                if !v.is_empty() {
                    if f.cfg.space.around_assignment {
                        l.push(' ');
                    }
                    l.push_str(v);
                }
                l.push(';');
                l
            } else {
                // 无初始化：原样
                let mut l = left;
                l.push(';');
                l
            };
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

/// 提取 `=` 对齐的行信息。
/// 返回 (kind, type_dim, name, value)。value 为 None 表示无 `=`（无初始化）。
fn eq_columns(
    f: &Formatter<'_>,
    node: CstNode<'_>,
) -> (&'static str, String, String, Option<String>) {
    match node.kind() {
        "data_declaration" | "net_declaration" => {
            let cols = declaration_columns(f, node);
            let mut name = cols[1].clone();
            if let Some(pos) = name.find('=') {
                let value = name[pos + 1..].trim().to_string();
                name = name[..pos].trim().to_string();
                ("decl", cols[0].clone(), name, Some(value))
            } else {
                // 无初始化：不输出 `=`（value 为 None），仅与其他行对齐名称
                ("decl", cols[0].clone(), name, None)
            }
        }
        "local_parameter_declaration" | "parameter_declaration" => {
            let (prefix, cols) = parameter_columns(f, node);
            ("param", prefix, cols[0].clone(), Some(cols[1].clone()))
        }
        "continuous_assign" => {
            let cols = assign_columns(f, node);
            (
                "assign",
                format!("assign {}", cols[0]),
                String::new(),
                Some(cols[2].clone()),
            )
        }
        _ => ("", node.text().to_string(), String::new(), None),
    }
}
