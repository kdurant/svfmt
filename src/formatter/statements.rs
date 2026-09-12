//! 语句布局：always/initial、seq_block、if/else、case、for、generate。

mod case;
mod control;

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::formatter::expressions::fmt_expr;
use crate::formatter::tokens::{display_width, has_newline};
use crate::parser::CstNode;

// 子模块对外接口 re-export（dispatch 与跨模块引用统一经此路径）
pub(crate) use case::{fmt_case_generate_construct, fmt_case_item, fmt_case_statement};
pub(crate) use control::{fmt_assert, fmt_body, fmt_conditional};

/// 解包 statement/statement_or_null/statement_item 包装，返回实际语句节点。
pub fn unwrap_statement<'a>(_f: &Formatter<'a>, node: CstNode<'a>) -> CstNode<'a> {
    let mut cur = node;
    loop {
        match cur.kind() {
            "statement_or_null"
            | "statement"
            | "statement_item"
            | "function_statement_or_null"
            | "function_statement" => {
                if let Some(c) = cur.named_child(0) {
                    cur = c;
                    continue;
                }
                return cur;
            }
            _ => return cur,
        }
    }
}

/// always/initial/final 构造。
pub fn fmt_procedural_construct(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let children: Vec<CstNode<'_>> = node.children();
    // 第一个是关键字（always/initial/final）
    let mut idx = 0;
    if let Some(first) = children.first()
        && (first.kind().ends_with("_keyword")
            || first.kind().starts_with("always")
            || first.kind().starts_with("initial")
            || first.kind().starts_with("final"))
    {
        docs.push(Doc::text(first.text()));
        idx = 1;
    }
    // 剩余：event_control 与/或 statement（body）
    // 注意：`initial begin ...` 的 body 是 statement_or_null（而非 statement），需一并识别
    let mut body: Option<CstNode<'_>> = None;
    while idx < children.len() {
        let c = children[idx];
        if c.is_named() && (c.kind() == "statement" || c.kind() == "statement_or_null") {
            body = Some(c);
            break;
        }
        // event_control 等
        if !docs.is_empty() && !matches!(docs.last(), Some(Doc::Space)) {
            docs.push(Doc::Space);
        }
        docs.push(f.fmt(c));
        idx += 1;
    }
    if let Some(b) = body {
        let inner = unwrap_statement(f, b);
        if inner.kind() == "procedural_timing_control_statement" {
            // 事件控制 + 语句体
            let sub: Vec<CstNode<'_>> = inner.children();
            let mut has_delay = false;
            for c in &sub {
                if (c.kind() == "event_control"
                    || c.kind() == "delay_control"
                    || c.kind() == "delay_value")
                    && c.kind().starts_with("delay")
                {
                    has_delay = true;
                    break;
                }
            }
            // 延时控制：仅当构造有前置关键字（如 `always #5 ...`）时另起一行；
            // 独立延时语句（如 seq 块内的 `#1us;`）无关键字，直接输出（由外层合并/换行）
            let has_keyword = !docs.is_empty();
            for c in &sub {
                if c.kind() == "event_control"
                    || c.kind() == "delay_control"
                    || c.kind() == "delay_value"
                {
                    if has_delay && has_keyword {
                        // 延迟控制另起一行（`always #5 ...`）
                        docs.push(Doc::Newline);
                        docs.push(Doc::Indent);
                    } else if !docs.is_empty() && !matches!(docs.last(), Some(Doc::Space)) {
                        docs.push(Doc::Space);
                    }
                    docs.push(f.fmt(*c));
                }
            }
            let body_child = sub
                .clone()
                .into_iter()
                .find(|c| c.kind() == "statement_or_null");
            if let Some(s) = body_child {
                let sb = unwrap_statement(f, s);
                if has_delay {
                    // `#(...)` 延迟控制：语句体同行
                    // 空语句体（如 `#1us;` 的 `;`）不加空格，直接跟分号
                    let empty_body = sb.kind() == "statement_or_null";
                    if !empty_body {
                        docs.push(Doc::Space);
                    }
                    docs.push(f.fmt(s));
                    if has_keyword {
                        docs.push(Doc::Dedent);
                    }
                } else {
                    if f.cfg.begin_end_on_newline {
                        docs.push(Doc::Newline);
                    } else {
                        docs.push(Doc::Space);
                    }
                    // 传入原始包装节点（statement_or_null）而非解包后的节点：
                    // fmt_body 内部依赖包装节点补 `;`（直接传 nonblocking_assignment
                    // 会走 fmt_expr 而丢失语句末尾分号，见 examples/timer.sv 的
                    // `enable_r <= {enable_r[0], enable};`）。
                    docs.push(fmt_body(f, s, 0));
                }
            }
        } else {
            if f.cfg.begin_end_on_newline {
                docs.push(Doc::Newline);
            } else {
                docs.push(Doc::Space);
            }
            docs.push(fmt_body(f, b, 0));
        }
    }
    Doc::concat(docs)
}

/// seq_block：begin ... end。
pub fn fmt_seq_block(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let children = node.children();
    // 第一个子节点是 `begin`
    let mut iter = children.into_iter();
    let begin = iter.next();
    let mut has_begin_trailing = false;
    if let Some(b) = begin
        && b.kind() == "begin"
    {
        docs.push(Doc::text("begin"));
        // 保留 begin 后原文行内空白（尾随空格）
        let rest: Vec<CstNode<'_>> = iter.collect();
        if let Some(first) = rest.first() {
            let ws = f.ws(b.byte_range().end, first.byte_range().start);
            let trailing = f.trailing_inline(ws);
            if !trailing.is_empty() {
                docs.push(Doc::text(trailing.to_string()));
                has_begin_trailing = true;
            }
        }
    }
    // 收集所有非 begin/end 的直接子节点（含注释），按顺序
    let all_children = node.children();
    let body_nodes: Vec<CstNode<'_>> = all_children
        .into_iter()
        .filter(|c| c.kind() != "begin" && c.kind() != "end")
        .collect();
    // begin 后：同行（单语句 + end_of_line_for_begin）或换行缩进
    let single_body = body_nodes.len() == 1;
    if f.cfg.end_of_line_for_begin && single_body {
        if !has_begin_trailing {
            docs.push(Doc::Space);
        }
    } else {
        docs.push(Doc::Newline);
        docs.push(Doc::Indent);
    }
    // begin 与第一个语句之间的空行
    if let (Some(b), Some(first)) = (begin, body_nodes.first()) {
        let ws = f.ws(b.byte_range().end, first.byte_range().start);
        let blanks = crate::formatter::count_blank_lines(ws);
        if blanks > 0 {
            docs.push(Doc::BlankLines(blanks));
        }
    }
    let body_docs = fmt_seq_block_body(f, &body_nodes);
    for d in body_docs {
        docs.push(d);
    }
    docs.push(Doc::Dedent);
    // end 前保留空行
    if let Some(last_body) = body_nodes.last() {
        let end_node = node.children().into_iter().find(|c| c.kind() == "end");
        if let Some(e) = end_node {
            let ws = f.ws(last_body.byte_range().end, e.byte_range().start);
            let blanks = crate::formatter::count_blank_lines(ws);
            if blanks > 0 {
                docs.push(Doc::BlankLines(blanks));
            } else {
                docs.push(Doc::Newline);
            }
        } else {
            docs.push(Doc::Newline);
        }
    } else {
        docs.push(Doc::Newline);
    }
    docs.push(Doc::text("end"));
    Doc::concat(docs)
}

/// 是否可参与 `=` 对齐的赋值语句。
///
/// 判据必须与 [`assignment_columns`] 完全一致：只有能提取出单条 `lhs op rhs` 的
/// 语句才可进入对齐段。否则 `emit_assign_segment` 取不出列，语句会被整条丢弃
/// ——例如 `a++;`（`inc_or_dec_expression`，无 rhs）曾被静默删除；
/// 含 ERROR 的赋值（`a = 1, b = 2;`）则会丢 token。
fn is_assignment_stmt(f: &Formatter<'_>, node: CstNode<'_>) -> bool {
    assignment_columns(unwrap_statement(f, node)).is_some()
}

/// seq_block 内语句：连续赋值对齐。
fn fmt_seq_block_body(f: &Formatter<'_>, body_nodes: &[CstNode<'_>]) -> Vec<Doc> {
    let mut docs: Vec<Doc> = Vec::new();
    let mut seg: Vec<CstNode<'_>> = Vec::new();
    let mut prev_end: Option<usize> = None;
    let mut first = true;

    let flush = |docs: &mut Vec<Doc>, seg: &mut Vec<CstNode<'_>>| {
        if seg.is_empty() {
            return;
        }
        // 确保赋值组前有换行
        if !docs.is_empty() && !matches!(docs.last(), Some(Doc::Newline) | Some(Doc::BlankLines(_)))
        {
            docs.push(Doc::Newline);
        }
        emit_assign_segment(f, seg, docs, true);
        seg.clear();
    };

    for node in body_nodes {
        let blank_before = prev_end
            .map(|pe| crate::formatter::count_blank_lines(f.ws(pe, node.byte_range().start)) > 0)
            .unwrap_or(false);
        if f.svdbg() {
            eprintln!(
                "[seqbody] kind={} blank_before={}",
                node.kind(),
                blank_before
            );
        }
        let is_assign = is_assignment_stmt(f, *node);
        if f.svdbg() {
            eprintln!(
                "[seqbody] kind={} is_assign={} text={:?}",
                node.kind(),
                is_assign,
                node.text().split('\n').next()
            );
        }
        let is_comment = node.is_named() && node.kind().ends_with("comment");
        if blank_before {
            let blanks = prev_end
                .map(|pe| crate::formatter::count_blank_lines(f.ws(pe, node.byte_range().start)))
                .unwrap_or(0);
            flush(&mut docs, &mut seg);
            if !docs.is_empty() && blanks > 0 {
                docs.push(Doc::BlankLines(blanks));
            }
        }
        if is_assign {
            // 前一个元素若不是赋值（且不是注释），断组
            if !seg.is_empty() && !blank_before {
                let prev_last = seg
                    .iter()
                    .rev()
                    .find(|n| !(n.is_named() && n.kind().ends_with("comment")));
                if let Some(pl) = prev_last
                    && !is_assignment_stmt(f, *pl)
                {
                    flush(&mut docs, &mut seg);
                }
            }
            seg.push(*node);
        } else if is_comment {
            if !seg.is_empty() && !blank_before {
                seg.push(*node);
            } else {
                flush(&mut docs, &mut seg);
                if !first {
                    docs.push(Doc::Newline);
                }
                // 独立注释：经 fmt 分派（块注释会规整内部缩进）
                docs.push(f.fmt(*node));
            }
        } else {
            // 延时语句且 delay_on_same_line：与上一赋值同行（无换行）时合并到前一行末尾
            let adjacent_delay = is_delay_stmt(f, *node)
                && f.cfg.delay_on_same_line
                && !blank_before
                && !seg.is_empty()
                && prev_end
                    .map(|pe| !has_newline(f.ws(pe, node.byte_range().start)))
                    .unwrap_or(false);
            if adjacent_delay {
                if !docs.is_empty()
                    && !matches!(docs.last(), Some(Doc::Newline) | Some(Doc::BlankLines(_)))
                {
                    docs.push(Doc::Newline);
                }
                emit_assign_segment(f, &seg, &mut docs, false); // 最后一行不换行
                seg.clear();
                docs.push(Doc::Space);
                docs.push(f.fmt(*node));
            } else {
                flush(&mut docs, &mut seg);
                if !first && !blank_before {
                    let ws = f.ws(prev_end.unwrap(), node.byte_range().start);
                    // delay_on_same_line=false 时独立延时强制换行（不残留前导空格）
                    if has_newline(ws) || (is_delay_stmt(f, *node) && !f.cfg.delay_on_same_line) {
                        let blanks = crate::formatter::count_blank_lines(ws);
                        if blanks > 0 {
                            docs.push(Doc::BlankLines(blanks));
                        } else {
                            docs.push(Doc::Newline);
                        }
                    } else {
                        docs.push(Doc::Space);
                    }
                }
                docs.push(f.fmt(*node));
            }
        }
        prev_end = Some(node.byte_range().end);
        first = false;
    }
    flush(&mut docs, &mut seg);
    docs
}

/// 是否为延时控制语句（如 `#1us;`）。
fn is_delay_stmt(f: &Formatter<'_>, node: CstNode<'_>) -> bool {
    let inner = unwrap_statement(f, node);
    if inner.kind() != "procedural_timing_control_statement" {
        return false;
    }
    inner.children().iter().any(|c| c.kind() == "delay_control")
}

/// task/function body：连续赋值对齐，其余语句保留原文（含源码缩进）。
///
/// 与 `fmt_seq_block_body` 不同：非赋值语句（if/for/自增等）不重新排版，
/// 而是保留源码原文与缩进（首行补源码起始列，内部行保留源码缩进），
/// 仅对连续赋值做 `=` 对齐。这样 task/function 体内的 if 等结构不会被
/// 拆成 begin/else 换行风格，保持用户手写布局。
fn fmt_task_function_body(f: &Formatter<'_>, body_nodes: &[CstNode<'_>]) -> Vec<Doc> {
    let mut docs: Vec<Doc> = Vec::new();
    let mut seg: Vec<CstNode<'_>> = Vec::new();
    let mut prev_end: Option<usize> = None;
    let mut first = true;

    let flush = |f: &Formatter<'_>, docs: &mut Vec<Doc>, seg: &mut Vec<CstNode<'_>>| {
        if seg.is_empty() {
            return;
        }
        // 确保赋值组前有换行
        if !docs.is_empty() && !matches!(docs.last(), Some(Doc::Newline) | Some(Doc::BlankLines(_)))
        {
            docs.push(Doc::Newline);
        }
        docs.push(Doc::Indent);
        emit_assign_segment(f, seg, docs, true);
        docs.push(Doc::Dedent);
        seg.clear();
    };

    for node in body_nodes {
        let blank_before = prev_end
            .map(|pe| crate::formatter::count_blank_lines(f.ws(pe, node.byte_range().start)) > 0)
            .unwrap_or(false);
        let inner = unwrap_statement(f, *node);
        let is_assign = assignment_columns(inner).is_some();
        let is_comment = node.is_named() && node.kind().ends_with("comment");
        if blank_before {
            let blanks = prev_end
                .map(|pe| crate::formatter::count_blank_lines(f.ws(pe, node.byte_range().start)))
                .unwrap_or(0);
            flush(f, &mut docs, &mut seg);
            if !docs.is_empty() && blanks > 0 {
                docs.push(Doc::BlankLines(blanks));
            }
        }
        if is_assign {
            // 前一个元素若不是赋值（且不是注释），断组
            if !seg.is_empty() && !blank_before {
                let prev_last = seg
                    .iter()
                    .rev()
                    .find(|n| !(n.is_named() && n.kind().ends_with("comment")));
                if let Some(pl) = prev_last
                    && assignment_columns(unwrap_statement(f, *pl)).is_none()
                {
                    flush(f, &mut docs, &mut seg);
                }
            }
            seg.push(*node);
        } else if is_comment {
            if !seg.is_empty() && !blank_before {
                seg.push(*node);
            } else {
                flush(f, &mut docs, &mut seg);
                if !first {
                    docs.push(Doc::Newline);
                }
                // 独立注释：经 fmt 分派（块注释会规整内部缩进）
                docs.push(f.fmt(*node));
            }
        } else {
            flush(f, &mut docs, &mut seg);
            if !first && !blank_before {
                let ws = f.ws(prev_end.unwrap(), node.byte_range().start);
                if has_newline(ws) {
                    let blanks = crate::formatter::count_blank_lines(ws);
                    if blanks > 0 {
                        docs.push(Doc::BlankLines(blanks));
                    } else {
                        docs.push(Doc::Newline);
                    }
                } else {
                    docs.push(Doc::Space);
                }
            }
            // 非赋值语句/声明/注释：原文保留，与赋值段一样缩进一级，内部行由
            // 渲染器按当前缩进统一重排。此前首行补"源码起始列"空格，会与当前
            // 缩进叠加，嵌套时缩进逐轮漂移（且与赋值段缩进级别不一致）。
            docs.push(Doc::Indent);
            docs.push(f.fmt_default(*node));
            docs.push(Doc::Dedent);
        }
        prev_end = Some(node.byte_range().end);
        first = false;
    }
    flush(f, &mut docs, &mut seg);
    docs
}

/// 输出赋值对齐组。
/// `trailing_newline` 为 false 时最后一行不换行（供相邻延时合并到该行）。
fn emit_assign_segment(
    f: &Formatter<'_>,
    seg: &[CstNode<'_>],
    docs: &mut Vec<Doc>,
    trailing_newline: bool,
) {
    // 列：lhs、op、rhs
    let tw = f.cfg.tab_width as usize;
    let mut rows: Vec<(String, String, CstNode<'_>)> = Vec::new();
    let mut max_lhs = 0usize;
    for node in seg {
        if node.is_named() && node.kind().ends_with("comment") {
            continue;
        }
        let inner = unwrap_statement(f, *node);
        if let Some((lhs, op, rhs)) = assignment_columns(inner) {
            let w = display_width(&lhs, tw);
            if w > max_lhs {
                max_lhs = w;
            }
            rows.push((lhs, op, rhs));
        } else if f.svdbg() {
            // 正常不会出现：进段的语句都满足 assignment_columns().is_some()
            eprintln!(
                "[assign-seg] non-assign in segment: inner_kind={} text={:?}",
                inner.kind(),
                inner.text()
            );
        }
    }
    // 对齐到 max_lhs + 1
    let op_col = max_lhs + 1;
    // 注释对齐列：块内最长行的 `;` 位置 + 3
    let block_max_semi = rows
        .iter()
        .map(|(lhs, op, rhs)| {
            display_width(lhs, tw).max(op_col)
                + display_width(op, tw)
                + 1
                + render_doc_width(f, *rhs)
                + 1
        })
        .max()
        .unwrap_or(0);
    if f.svdbg() {
        eprintln!(
            "[assign-seg] rows={:?} max_lhs={} op_col={}",
            rows.iter()
                .map(|(l, o, _)| (l.clone(), o.clone()))
                .collect::<Vec<_>>(),
            max_lhs,
            op_col
        );
    }
    // 行文本按块内相对列对齐（见 pad_comment_rel：输出可能被上层再次渲染为
    // 字符串，绝对列不可知，且不能依赖源码缩进）。
    let mut lines: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for node in seg {
        if node.is_named() && node.kind().ends_with("comment") {
            if let Some(l) = current.take() {
                let ws = if let Some(pe) = prev_node_end(seg, *node) {
                    f.ws(pe, node.byte_range().start)
                } else {
                    ""
                };
                if !ws.contains('\n') {
                    // 块内相对对齐（不读源码缩进：那会随输入变化，破坏幂等性）
                    lines.push(crate::formatter::module::pad_comment_rel(
                        f,
                        &l,
                        node.text(),
                        block_max_semi,
                    ));
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
            let inner = unwrap_statement(f, *node);
            if let Some((lhs, op, rhs)) = assignment_columns(inner) {
                let mut line = String::new();
                let mut cell = lhs.clone();
                if f.cfg.align_assignments {
                    while display_width(&cell, tw) < op_col {
                        cell.push(' ');
                    }
                } else {
                    cell.push(' ');
                }
                line.push_str(&cell);
                line.push_str(&op);
                if f.cfg.space.around_assignment {
                    line.push(' ');
                }
                let rhs_doc = fmt_expr(f, rhs, &crate::formatter::expressions::ExprCtx::default());
                let rhs_text = render_doc(f, rhs_doc);
                line.push_str(&rhs_text);
                line.push(';');
                current = Some(line);
            } else {
                // 取不出 `lhs op rhs`（分组判据与提取不一致的兜底）：原样输出，
                // 绝不能静默丢弃语句。
                if f.svdbg() {
                    eprintln!(
                        "[assign-seg] fallback raw inner_kind={} text={:?}",
                        inner.kind(),
                        inner.text()
                    );
                }
                lines.push(render_doc(f, f.fmt(*node)));
            }
        }
    }
    if let Some(l) = current {
        lines.push(l);
    }
    for (i, line) in lines.iter().enumerate() {
        docs.push(Doc::text(line.clone()));
        if i + 1 < lines.len() || trailing_newline {
            docs.push(Doc::Newline);
        }
    }
}

fn render_doc_width(f: &Formatter<'_>, node: CstNode<'_>) -> usize {
    let doc = fmt_expr(f, node, &crate::formatter::expressions::ExprCtx::default());
    display_width(&render_doc(f, doc), f.cfg.tab_width as usize)
}

/// 把 Doc 渲染为文本（用于 rhs，强制单行不受 column_limit 影响）。
fn render_doc(f: &Formatter<'_>, doc: Doc) -> String {
    crate::document::render_inline(&doc, f.cfg)
}

fn prev_node_end(seg: &[CstNode<'_>], node: CstNode<'_>) -> Option<usize> {
    for (i, n) in seg.iter().enumerate() {
        if n.byte_range().start == node.byte_range().start {
            if i > 0 {
                return Some(seg[i - 1].byte_range().end);
            }
            return None;
        }
    }
    None
}

/// 提取赋值的 lhs / op / rhs 节点。
///
/// 含语法错误（ERROR）的赋值返回 `None`：ERROR 恢复出的节点结构不可靠，
/// 按此提取会丢失 token（如 `a = 1, b = 2;` 的 `a`）。调用方据此回退到原文输出。
fn assignment_columns(node: CstNode<'_>) -> Option<(String, String, CstNode<'_>)> {
    if node.subtree_has_error() {
        return None;
    }
    // blocking_assignment 内嵌 operator_assignment
    let mut node = node;
    if node.kind() == "blocking_assignment" {
        if let Some(oa) = node.find_named_child("operator_assignment") {
            node = oa;
        } else if crate::formatter::svdbg() {
            eprintln!(
                "[assign-cols] operator_assignment NOT FOUND, children={:?}",
                node.children().iter().map(|c| c.kind()).collect::<Vec<_>>()
            );
        }
    }
    let mut lhs = String::new();
    let mut op = String::new();
    let mut rhs: Option<CstNode<'_>> = None;
    for c in node.children_iter() {
        if c.kind() == "=" || c.kind() == "<=" || c.kind() == "assignment_operator" {
            op = c.text().to_string();
        } else if op.is_empty() {
            lhs = c.text().to_string();
        } else {
            rhs = Some(c);
        }
    }
    if crate::formatter::svdbg() && node.kind() == "operator_assignment" {
        eprintln!(
            "[assign-cols] oa children={:?} lhs={:?} op={:?} rhs_is={}",
            node.children().iter().map(|c| c.kind()).collect::<Vec<_>>(),
            lhs,
            op,
            rhs.is_some()
        );
    }
    let rhs = rhs?;
    if op.is_empty() {
        if crate::formatter::svdbg() {
            eprintln!("[assign-cols] op empty, lhs={:?}", lhs);
        }
        return None;
    }
    Some((lhs, op, rhs))
}

/// loop_statement：`for` / `while` / `do...while` / `repeat` / `forever` / `foreach`。
///
/// 头部（关键字 + `(...)`）与 body 按位置切分：
/// - `for`/`while`/`repeat`/`foreach`：头部到头一个配对 `)` 为止；
/// - `do`/`forever`：头部仅关键字本身；
/// - `do` 的 `while (cond);` 属于 body 之后的**尾部**，必须保留（否则条件丢失）。
///
/// body 不再限定为 `statement_or_null`/`generate_block`：`foreach`、`do` 的 body
/// 可能是裸语句节点，此前会被静默跳过导致整个循环体丢失。
pub fn fmt_loop_statement(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let items: Vec<CstNode<'_>> = node.children();
    if items.is_empty() {
        return f.raw(node);
    }
    // 头部结束位置（body 起始下标）
    let head_kw = items[0].kind();
    let mut header_end = 1usize;
    if !matches!(head_kw, "do" | "forever") && items.get(1).is_some_and(|c| c.kind() == "(") {
        let mut depth = 0usize;
        for (k, c) in items.iter().enumerate().skip(1) {
            match c.kind() {
                "(" => depth += 1,
                ")" => {
                    depth -= 1;
                    if depth == 0 {
                        header_end = k + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    let mut docs: Vec<Doc> = Vec::new();
    for (k, c) in items.iter().enumerate().take(header_end) {
        match c.kind() {
            "for" | "while" | "do" | "repeat" | "forever" | "foreach" => {
                docs.push(Doc::text(c.text()));
                if f.cfg.space.before_control_statement_parens
                    && items.get(k + 1).is_some_and(|n| n.kind() == "(")
                {
                    docs.push(Doc::Space);
                }
            }
            "(" => docs.push(Doc::text("(")),
            ")" => docs.push(Doc::text(")")),
            ";" => {
                docs.push(Doc::text(";"));
                // 分号后空格（AfterSemicolon）
                if f.cfg.space.after_semicolon {
                    docs.push(Doc::Space);
                }
            }
            // 其它头部节点（for 的 init/cond/step、foreach 的 loop_variables 等）
            _ => docs.push(fmt_expr(
                f,
                *c,
                &crate::formatter::expressions::ExprCtx::default(),
            )),
        }
    }
    // body
    let mut idx = header_end;
    if let Some(b) = items.get(idx).copied() {
        let inner = unwrap_statement(f, b);
        if inner.kind() == "generate_block" || inner.kind() == "seq_block" {
            docs.push(Doc::Newline);
            docs.push(fmt_seq_block_with_label(f, inner));
        } else {
            docs.push(Doc::Newline);
            docs.push(match inner.kind() {
                "conditional_statement" => fmt_conditional(f, inner),
                _ => Doc::concat(vec![Doc::Indent, f.fmt(b), Doc::Dedent]),
            });
        }
        idx += 1;
        // 裸语句 body（`do i++;`、`foreach (...) x = i;`）的结束分号是兄弟节点
        if items.get(idx).is_some_and(|c| c.kind() == ";") {
            docs.push(Doc::text(";"));
            idx += 1;
        }
    }
    // body 之后的尾部：`do ... while (cond);` 的 while 条件
    let mut first_tail = true;
    for c in items.iter().skip(idx) {
        match c.kind() {
            "while" => {
                if first_tail {
                    docs.push(Doc::Newline);
                }
                docs.push(Doc::text("while"));
                if f.cfg.space.before_control_statement_parens {
                    docs.push(Doc::Space);
                }
            }
            "(" => docs.push(Doc::text("(")),
            ")" => docs.push(Doc::text(")")),
            ";" => docs.push(Doc::text(";")),
            _ => docs.push(fmt_expr(
                f,
                *c,
                &crate::formatter::expressions::ExprCtx::default(),
            )),
        }
        first_tail = false;
    }
    Doc::concat(docs)
}

/// par_block：`fork [: label] ... join|join_any|join_none [: label]`。
///
/// 此前 `par_block` 无处理分支，落到原文输出：多行原文的内部缩进会被渲染器
/// 再次叠加，导致多次格式化缩进逐轮增长（幂等性破坏）。
pub fn fmt_par_block(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let children: Vec<CstNode<'_>> = node.children();
    let mut docs: Vec<Doc> = Vec::new();
    let mut idx = 0usize;
    // `fork [: label]`
    if children.first().is_some_and(|c| c.kind() == "fork") {
        docs.push(Doc::text("fork"));
        idx = 1;
        if children.get(idx).is_some_and(|c| c.kind() == ":")
            && children
                .get(idx + 1)
                .is_some_and(|c| c.kind() == "simple_identifier")
        {
            docs.push(Doc::text(" : "));
            docs.push(Doc::text(children[idx + 1].text()));
            idx += 2;
        }
    }
    // 结束关键字节点的 kind 是 `join_keyword`（文本为 join/join_any/join_none）
    let end_idx = children.iter().position(|c| c.kind() == "join_keyword");
    let body_end = end_idx.unwrap_or(children.len());
    let body: Vec<CstNode<'_>> = children[idx.min(body_end)..body_end].to_vec();
    docs.push(Doc::Newline);
    docs.push(Doc::Indent);
    for d in fmt_seq_block_body(f, &body) {
        docs.push(d);
    }
    docs.push(Doc::Dedent);
    docs.push(Doc::Newline);
    if let Some(ei) = end_idx {
        docs.push(Doc::text(children[ei].text()));
        // `join : label`
        if children.get(ei + 1).is_some_and(|c| c.kind() == ":")
            && children
                .get(ei + 2)
                .is_some_and(|c| c.kind() == "simple_identifier")
        {
            docs.push(Doc::text(" : "));
            docs.push(Doc::text(children[ei + 2].text()));
        }
    }
    Doc::concat(docs)
}

/// begin ... end 块（支持 `begin : label`）。
pub fn fmt_seq_block_with_label(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let children: Vec<CstNode<'_>> = node.children();
    let mut docs: Vec<Doc> = Vec::new();
    let mut begin_node: Option<CstNode<'_>> = None;
    let mut idx = 0usize;
    if let Some(b) = children.first()
        && b.kind() == "begin"
    {
        docs.push(Doc::text("begin"));
        begin_node = Some(*b);
        idx = 1;
    }
    // `: label`
    if f.svdbg() {
        eprintln!(
            "[blk] children={:?}",
            children.iter().map(|c| c.kind()).collect::<Vec<_>>()
        );
    }
    while idx + 1 < children.len() {
        if children[idx].kind() == ":" && children[idx + 1].kind() == "simple_identifier" {
            docs.push(Doc::text(" : "));
            docs.push(Doc::text(children[idx + 1].text()));
            idx += 2;
        } else {
            break;
        }
    }
    let has_label = has_colon_label(&children);
    if f.svdbg() {
        eprintln!("[blk] has_label={}", has_label);
    }
    let body_nodes: Vec<CstNode<'_>> = children
        .iter()
        .enumerate()
        .filter(|(i, c)| {
            if c.kind() == "begin" || c.kind() == "end" || c.kind() == ":" {
                return false;
            }
            if has_label && c.kind() == "simple_identifier" {
                // 跳过 `: label`
                if i > &0 && children[i - 1].kind() == ":" {
                    return false;
                }
            }
            true
        })
        .map(|(_, c)| *c)
        .collect();
    if let (Some(b), Some(first)) = (begin_node, body_nodes.first()) {
        // 从 begin 或 label 之后的空白取行内尾随空格
        let anchor_end = if idx > 0 && idx <= children.len() {
            children[idx - 1].byte_range().end
        } else {
            b.byte_range().end
        };
        let ws = f.ws(anchor_end, first.byte_range().start);
        let trailing = f.trailing_inline(ws);
        if !trailing.is_empty() {
            docs.push(Doc::text(trailing.to_string()));
        }
    }
    docs.push(Doc::Newline);
    docs.push(Doc::Indent);
    let body_docs = fmt_seq_block_body(f, &body_nodes);
    for d in body_docs {
        docs.push(d);
    }
    docs.push(Doc::Dedent);
    docs.push(Doc::Newline);
    docs.push(Doc::text("end"));
    Doc::concat(docs)
}

/// generate 区域。
pub fn fmt_generate(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    // generate_block（begin...end）：内部连续 assign 走模块体对齐段逻辑
    if node.kind() == "generate_block" {
        return fmt_generate_block(f, node);
    }
    let mut docs: Vec<Doc> = Vec::new();
    let children = node.children();
    let mut prev_end: Option<usize> = None;
    let mut first = true;
    for child in children {
        let is_kw = matches!(child.kind(), "generate" | "endgenerate" | "begin" | "end");
        if !first {
            let ws = f.ws(prev_end.unwrap(), child.byte_range().start);
            let blanks = crate::formatter::count_blank_lines(ws);
            if blanks > 0 {
                docs.push(Doc::BlankLines(blanks));
            } else if has_newline(ws) {
                docs.push(Doc::Newline);
            } else {
                docs.push(Doc::Space);
            }
        }
        if !is_kw {
            docs.push(Doc::Indent);
        }
        docs.push(f.fmt(child));
        if !is_kw {
            docs.push(Doc::Dedent);
        }
        prev_end = Some(child.byte_range().end);
        first = false;
    }
    Doc::concat(docs)
}

/// generate_block（begin ... end）：连续 assign 对齐段（复用模块体 emit_aligned_segment）。
fn fmt_generate_block(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let children: Vec<CstNode<'_>> = node.children();
    let mut prev_end: Option<usize> = None;
    let mut first = true;
    let mut seg: Vec<CstNode<'_>> = Vec::new();

    let flush = |f: &Formatter<'_>, seg: &mut Vec<CstNode<'_>>, docs: &mut Vec<Doc>| {
        if seg.is_empty() {
            return;
        }
        if !docs.is_empty() && !matches!(docs.last(), Some(Doc::Newline) | Some(Doc::BlankLines(_)))
        {
            docs.push(Doc::Newline);
        }
        docs.push(Doc::Indent);
        crate::formatter::module::emit_aligned_segment(f, seg, docs);
        docs.push(Doc::Dedent);
        seg.clear();
    };

    for child in children {
        let is_kw = matches!(child.kind(), "begin" | "end");
        // 多赋值 / 含 ERROR 的赋值不参与对齐段（对齐只保留一条赋值，会丢语义）
        let is_assign = child.kind() == "continuous_assign"
            && crate::formatter::module::continuous_assign_alignable(child);

        let (blank_before, has_nl, blanks) = if let Some(pe) = prev_end {
            let ws = f.ws(pe, child.byte_range().start);
            let b = crate::formatter::count_blank_lines(ws);
            (b > 0, has_newline(ws), b)
        } else {
            (false, false, 0)
        };

        if is_assign && !blank_before {
            // 连续 assign：收集进对齐段
            if seg.is_empty() && !first {
                // 段起始前需换行
                docs.push(Doc::Newline);
            }
            seg.push(child);
            prev_end = Some(child.byte_range().end);
            first = false;
            continue;
        }

        // 非 assign：先冲刷对齐段
        flush(f, &mut seg, &mut docs);

        if !first {
            let ws = prev_end
                .map(|pe| f.ws(pe, child.byte_range().start))
                .unwrap_or("");
            let _ = ws;
            if blank_before {
                docs.push(Doc::BlankLines(blanks));
            } else if has_nl {
                docs.push(Doc::Newline);
            } else if child.kind() == ";" {
                // 裸分号（如 ERROR 表达式后）：紧跟前一个 token，不加空格
            } else {
                docs.push(Doc::Space);
            }
        }
        if !is_kw {
            docs.push(Doc::Indent);
        }
        docs.push(f.fmt(child));
        if !is_kw {
            docs.push(Doc::Dedent);
        }
        prev_end = Some(child.byte_range().end);
        first = false;
    }
    flush(f, &mut seg, &mut docs);
    Doc::concat(docs)
}

/// if_generate_construct：`if (cond) begin...end [else begin...end]`。
pub fn fmt_if_generate_construct(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();
    let mut body: Option<CstNode<'_>> = None;
    let mut after_body = 0usize;

    let mut i = 0usize;
    while i < items.len() {
        let c = items[i];
        match c.kind() {
            "if" => {
                docs.push(Doc::text("if"));
                if f.cfg.space.before_control_statement_parens {
                    docs.push(Doc::Space);
                }
            }
            "(" => docs.push(Doc::text("(")),
            ")" => docs.push(Doc::text(")")),
            "constant_expression" | "expression" | "unary_expression" | "primary" => {
                docs.push(fmt_expr(
                    f,
                    c,
                    &crate::formatter::expressions::ExprCtx::default(),
                ));
            }
            "generate_block" => {
                body = Some(c);
                after_body = i + 1;
                break;
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(b) = body {
        docs.push(Doc::Newline);
        docs.push(fmt_generate_block(f, b));
        // else 子句：`else [comment] begin...end`
        let mut j = after_body;
        while j < items.len() {
            let c = items[j];
            if c.kind() == "else" {
                docs.push(Doc::Newline);
                docs.push(Doc::text("else"));
                // else 后的行内注释
                if let Some(next) = items.get(j + 1)
                    && next.is_named()
                    && next.kind().ends_with("comment")
                    && !f
                        .ws(c.byte_range().end, next.byte_range().start)
                        .contains('\n')
                {
                    docs.push(Doc::Space);
                    docs.push(Doc::text(next.text()));
                    j += 1;
                }
                if let Some(gb) = items.get(j + 1)
                    && gb.kind() == "generate_block"
                {
                    docs.push(Doc::Newline);
                    docs.push(fmt_generate_block(f, *gb));
                }
                break;
            }
            j += 1;
        }
    }
    Doc::concat(docs)
}

pub fn has_colon_label(children: &[CstNode<'_>]) -> bool {
    children
        .windows(2)
        .any(|w| w[0].kind() == ":" && w[1].kind() == "simple_identifier")
}

/// task_declaration / function_declaration：task/function 头原文输出，body 中
/// 连续赋值对齐、其余语句保留原文，endtask/endfunction 与 task/function 对齐
/// （当前缩进级别）。
///
/// 此前这些节点走 `fmt_default` 输出整段原文，body 内部行保留源码缩进（含未对齐
/// 的赋值），且 endtask/endfunction 也保留了源码缩进，导致与 task/function 不对齐。
/// 这里 body 经 `fmt_task_function_body`（赋值对齐、非赋值语句保留原文），
/// endtask/endfunction 单独输出在当前缩进级别，与 task/function 对齐。
pub fn fmt_task_or_function_declaration(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let (body_kind, end_kind) = if node.kind() == "task_declaration" {
        ("task_body_declaration", "endtask")
    } else {
        ("function_body_declaration", "endfunction")
    };
    let Some(body) = node.children().into_iter().find(|c| c.kind() == body_kind) else {
        return f.raw(node);
    };
    let b_children: Vec<CstNode<'_>> = body.children();
    // 头部：到第一个括号深度为 0 的 `;` 为止（含）。其后直到 endtask/endfunction
    // 之间的**全部**子节点都属于 body——包括声明、注释与 ERROR 恢复节点。
    // 此前只收集 `statement_or_null` 系节点，导致函数体内的声明与注释被静默
    // 丢弃（如 `int y;` 只剩 `y;`）。
    let mut depth = 0usize;
    let mut header_end: Option<(usize, usize)> = None;
    for (k, c) in b_children.iter().enumerate() {
        match c.kind() {
            "(" => depth += 1,
            ")" => depth = depth.saturating_sub(1),
            ";" if depth == 0 => {
                header_end = Some((k, c.byte_range().end));
                break;
            }
            _ => {}
        }
    }
    let Some((h_idx, h_end)) = header_end else {
        return f.raw(node);
    };
    // 头：从节点起点到头部 `;`（原文，保留多行端口列表）
    let header_raw = &f.src[node.byte_range().start..h_end];
    let header = header_raw.trim_end();
    let body_items: Vec<CstNode<'_>> = b_children[h_idx + 1..]
        .iter()
        .filter(|c| c.kind() != end_kind)
        .copied()
        .collect();
    // 头与第一个 body 元素之间的空行
    let first_start = body_items.first().map_or(h_end, |c| c.byte_range().start);
    let blanks = crate::formatter::count_blank_lines(&f.src[h_end..first_start]);
    let mut docs = vec![Doc::text(header.to_string())];
    docs.push(Doc::Newline);
    if blanks > 0 {
        docs.push(Doc::BlankLines(blanks));
    }
    docs.extend(fmt_task_function_body(f, &body_items));
    docs.push(Doc::Newline);
    docs.push(Doc::text(end_kind));
    Doc::concat(docs)
}
