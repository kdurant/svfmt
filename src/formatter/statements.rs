//! 语句布局：always/initial、seq_block、if/else、case、for、generate。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::formatter::expressions::fmt_expr;
use crate::formatter::tokens::has_newline;
use crate::parser::CstNode;

/// 解包 statement/statement_or_null/statement_item 包装，返回实际语句节点。
pub fn unwrap_statement<'a>(_f: &Formatter<'a>, node: CstNode<'a>) -> CstNode<'a> {
    let mut cur = node;
    loop {
        match cur.kind() {
            "statement_or_null" | "statement" | "statement_item" => {
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
    let mut body: Option<CstNode<'_>> = None;
    while idx < children.len() {
        let c = children[idx];
        if c.is_named() && c.kind() == "statement" {
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
            for c in &sub {
                if c.kind() == "event_control"
                    || c.kind() == "delay_control"
                    || c.kind() == "delay_value"
                {
                    if has_delay {
                        // 延迟控制另起一行
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
                    docs.push(Doc::Space);
                    docs.push(f.fmt(s));
                    docs.push(Doc::Dedent);
                } else {
                    docs.push(Doc::Newline);
                    docs.push(fmt_body(f, sb, 0));
                }
            }
        } else {
            docs.push(Doc::Newline);
            docs.push(fmt_body(f, inner, 0));
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
            }
        }
    }
    docs.push(Doc::Newline);
    docs.push(Doc::Indent);
    // 收集所有非 begin/end 的直接子节点（含注释），按顺序
    let all_children = node.children();
    let body_nodes: Vec<CstNode<'_>> = all_children
        .into_iter()
        .filter(|c| c.kind() != "begin" && c.kind() != "end")
        .collect();
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

/// 是否赋值语句（unwrap 后为 blocking/nonblocking/operator assignment）。
fn is_assignment_stmt(f: &Formatter<'_>, node: CstNode<'_>) -> bool {
    let inner = unwrap_statement(f, node);
    matches!(
        inner.kind(),
        "blocking_assignment" | "nonblocking_assignment" | "operator_assignment"
    )
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
        emit_assign_segment(f, seg, docs);
        seg.clear();
    };

    for node in body_nodes {
        let blank_before = prev_end
            .map(|pe| crate::formatter::count_blank_lines(f.ws(pe, node.byte_range().start)) > 0)
            .unwrap_or(false);
        if std::env::var("SVDBG").is_ok() {
            eprintln!(
                "[seqbody] kind={} blank_before={}",
                node.kind(),
                blank_before
            );
        }
        let is_assign = is_assignment_stmt(f, *node);
        if std::env::var("SVDBG").is_ok() {
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
                let prev_last = seg.last().unwrap();
                if !is_assignment_stmt(f, *prev_last) {
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
                docs.push(Doc::text(node.text().to_string()));
            }
        } else {
            flush(&mut docs, &mut seg);
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
            docs.push(f.fmt(*node));
        }
        prev_end = Some(node.byte_range().end);
        first = false;
    }
    flush(&mut docs, &mut seg);
    docs
}

/// 输出赋值对齐组。
fn emit_assign_segment(f: &Formatter<'_>, seg: &[CstNode<'_>], docs: &mut Vec<Doc>) {
    // 列：lhs、op、rhs
    let mut rows: Vec<(String, String, CstNode<'_>)> = Vec::new();
    let mut max_lhs = 0usize;
    for node in seg {
        if node.is_named() && node.kind().ends_with("comment") {
            continue;
        }
        let inner = unwrap_statement(f, *node);
        if let Some((lhs, op, rhs)) = assignment_columns(inner) {
            let w = lhs.chars().count();
            if w > max_lhs {
                max_lhs = w;
            }
            rows.push((lhs, op, rhs));
        } else if std::env::var("SVDBG").is_ok() {
            eprintln!(
                "[assign-seg] DROPPED inner_kind={} text={:?}",
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
            lhs.chars().count().max(op_col) + op.chars().count() + 1 + render_doc_width(f, *rhs) + 1
        })
        .max()
        .unwrap_or(0);
    if std::env::var("SVDBG").is_ok() {
        eprintln!(
            "[assign-seg] rows={:?} max_lhs={} op_col={}",
            rows.iter()
                .map(|(l, o, _)| (l.clone(), o.clone()))
                .collect::<Vec<_>>(),
            max_lhs,
            op_col
        );
    }
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
                    let base_indent = seg
                        .first()
                        .map(|n| n.byte_range().start)
                        .and_then(|s| f.src[..s].rfind('\n'))
                        .map(|pos| f.src[pos + 1..seg.first().unwrap().byte_range().start].len())
                        .unwrap_or(0);
                    lines.push(crate::formatter::module::pad_comment_pub(
                        f,
                        &l,
                        node.text(),
                        base_indent,
                        block_max_semi,
                    ));
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
            let inner = unwrap_statement(f, *node);
            if let Some((lhs, op, rhs)) = assignment_columns(inner) {
                let mut line = String::new();
                let mut cell = lhs.clone();
                while cell.chars().count() < op_col {
                    cell.push(' ');
                }
                line.push_str(&cell);
                line.push_str(&op);
                line.push(' ');
                let rhs_doc = fmt_expr(f, rhs, &crate::formatter::expressions::ExprCtx::default());
                let rhs_text = render_doc(f, rhs_doc);
                line.push_str(&rhs_text);
                line.push(';');
                current = Some(line);
            }
        }
    }
    if let Some(l) = current {
        lines.push(l);
    }
    for line in &lines {
        docs.push(Doc::text(line.clone()));
        docs.push(Doc::Newline);
    }
}

fn render_doc_width(f: &Formatter<'_>, node: CstNode<'_>) -> usize {
    let doc = fmt_expr(f, node, &crate::formatter::expressions::ExprCtx::default());
    render_doc(f, doc).chars().count()
}

/// 把 Doc 渲染为文本（用于 rhs）。
fn render_doc(f: &Formatter<'_>, doc: Doc) -> String {
    let opts = crate::document::RenderOptions::from(f.cfg);
    crate::document::render(&doc, &opts)
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
fn assignment_columns(node: CstNode<'_>) -> Option<(String, String, CstNode<'_>)> {
    // blocking_assignment 内嵌 operator_assignment
    let mut node = node;
    if node.kind() == "blocking_assignment" {
        if let Some(oa) = node.find_named_child("operator_assignment") {
            node = oa;
        } else if std::env::var("SVDBG").is_ok() {
            eprintln!(
                "[assign-cols] operator_assignment NOT FOUND, children={:?}",
                node.children().iter().map(|c| c.kind()).collect::<Vec<_>>()
            );
        }
    }
    let mut lhs = String::new();
    let mut op = String::new();
    let mut rhs: Option<CstNode<'_>> = None;
    for c in node.children() {
        if c.kind() == "=" || c.kind() == "<=" || c.kind() == "assignment_operator" {
            op = c.text().to_string();
        } else if op.is_empty() {
            lhs = c.text().to_string();
        } else {
            rhs = Some(c);
        }
    }
    if std::env::var("SVDBG").is_ok() && node.kind() == "operator_assignment" {
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
        if std::env::var("SVDBG").is_ok() {
            eprintln!("[assign-cols] op empty, lhs={:?}", lhs);
        }
        return None;
    }
    Some((lhs, op, rhs))
}

/// conditional_statement：if/else if/else。
pub fn fmt_conditional(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();
    let mut is_else = false;
    let mut cond_after_else = false;
    for (i, child) in items.iter().enumerate() {
        match child.kind() {
            "if" => {
                if is_else {
                    docs.push(Doc::Space);
                }
                docs.push(Doc::text("if"));
                cond_after_else = false;
            }
            "else" => {
                docs.push(Doc::Newline);
                docs.push(Doc::text("else"));
                is_else = true;
                cond_after_else = false;
            }
            "cond_predicate" => {
                docs.push(crate::formatter::module::fmt_cond_expression(f, *child));
                docs.push(Doc::Newline);
                cond_after_else = true;
            }
            "statement_or_null" => {
                if is_else && !cond_after_else {
                    docs.push(Doc::Newline);
                }
                let inner = unwrap_statement(f, *child);
                if matches!(inner.kind(), "seq_block" | "conditional_statement") {
                    docs.push(fmt_body(f, *child, 0));
                } else {
                    // 单语句：检查其后的行尾注释
                    let trailing_comment = items.get(i + 1).filter(|n| {
                        n.is_named()
                            && n.kind().ends_with("comment")
                            && !f
                                .ws(child.byte_range().end, n.byte_range().start)
                                .contains('\n')
                    });
                    let body_doc = f.fmt(*child);
                    if let Some(cmt) = trailing_comment {
                        let text = render_doc(f, body_doc);
                        let semi_rel = text.rfind(';').unwrap_or(0);
                        // 语句内行尾注释：对齐到 semi+3（相对行首），禁用 40 下限
                        let base_indent = f.cfg.comment_column as usize;
                        let padded = crate::formatter::module::pad_comment_pub(
                            f,
                            &text,
                            cmt.text(),
                            base_indent,
                            semi_rel,
                        );
                        docs.push(Doc::Indent);
                        docs.push(Doc::text(padded));
                        docs.push(Doc::Dedent);
                    } else {
                        docs.push(fmt_body(f, *child, 0));
                    }
                }
                is_else = false;
                cond_after_else = false;
            }
            _ => {}
        }
    }
    Doc::concat(docs)
}

/// 语句体：seq_block 换行 begin；单语句缩进。
pub fn fmt_body(f: &Formatter<'_>, node: CstNode<'_>, _depth: usize) -> Doc {
    let inner = unwrap_statement(f, node);
    match inner.kind() {
        "seq_block" => fmt_seq_block(f, inner),
        "conditional_statement" => fmt_conditional(f, inner),
        _ => {
            // 单语句：用包装节点格式化（补齐 `;`）
            let body_doc = f.fmt(node);
            Doc::concat(vec![Doc::Indent, body_doc, Doc::Dedent])
        }
    }
}

/// case_statement。
pub fn fmt_case_statement(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();
    // case 关键字 + 表达式
    let mut i = 0;
    while i < items.len() {
        let c = items[i];
        if c.kind() == "case_keyword" || c.kind() == "casez_keyword" || c.kind() == "casex_keyword"
        {
            docs.push(Doc::text(c.text()));
            i += 1;
            continue;
        }
        if c.kind() == "(" {
            docs.push(Doc::text("("));
            i += 1;
            continue;
        }
        if c.kind() == ")" {
            docs.push(Doc::text(")"));
            i += 1;
            continue;
        }
        if c.kind() == "case_expression" {
            docs.push(fmt_expr(
                f,
                c,
                &crate::formatter::expressions::ExprCtx::default(),
            ));
            i += 1;
            continue;
        }
        break;
    }
    // 收集 case_item 与注释（按顺序）
    let mut events: Vec<CstNode<'_>> = Vec::new();
    while i < items.len() {
        let c = items[i];
        if c.kind() == "endcase" {
            break;
        }
        if c.kind() == "case_item" || (c.is_named() && c.kind().ends_with("comment")) {
            events.push(c);
        }
        i += 1;
    }
    // 计算对齐宽度（表达式文本）
    let items_only: Vec<CstNode<'_>> = events
        .iter()
        .filter(|c| c.kind() == "case_item")
        .copied()
        .collect();
    let expr_widths: Vec<usize> = items_only
        .iter()
        .map(|c| case_item_expr_text(f, *c).chars().count())
        .collect();
    // 若 item 的 body 风格不一致（单行 / seq_block / conditional），则不对齐
    let body_styles: Vec<&str> = items_only
        .iter()
        .map(|c| case_item_body_style(f, *c))
        .collect();
    let mixed = body_styles.iter().any(|a| *a != body_styles[0]);
    let max_w = if mixed {
        0
    } else {
        expr_widths.iter().copied().max().unwrap_or(0)
    };
    docs.push(Doc::Newline);
    docs.push(Doc::Indent);
    // 重新构建行：注释同行则合并
    let mut out_texts: Vec<Option<String>> = Vec::new();
    let mut current: Option<String> = None;
    for c in &events {
        if c.kind() == "case_item" {
            if let Some(l) = current.take() {
                out_texts.push(Some(l));
            }
            let doc = fmt_case_item(f, *c, &[max_w]);
            let text = render_doc(f, doc);
            if text.contains('\n') {
                out_texts.push(Some(text));
            } else {
                current = Some(text);
            }
        } else {
            // 注释
            if let Some(l) = current.take() {
                // 合并到上一个单行 case_item
                let cmt_text = c.text();
                let prev_event = events
                    .iter()
                    .rfind(|e| e.byte_range().end <= c.byte_range().start);
                let ws = prev_event
                    .map(|e| f.ws(e.byte_range().end, c.byte_range().start))
                    .unwrap_or("");
                if !ws.contains('\n') {
                    let semi_rel = l.rfind(';').unwrap_or(0);
                    let padded = crate::formatter::module::pad_comment_pub(
                        f,
                        &l,
                        cmt_text,
                        f.cfg.comment_column as usize,
                        semi_rel,
                    );
                    out_texts.push(Some(padded));
                } else {
                    out_texts.push(Some(l));
                    out_texts.push(Some(cmt_text.to_string()));
                }
            } else {
                out_texts.push(Some(c.text().to_string()));
            }
        }
    }
    if let Some(l) = current {
        out_texts.push(Some(l));
    }
    // 输出行，行间换行/空行（依据事件原文空白）
    let mut text_idx = 0usize;
    let mut prev_end: Option<usize> = None;
    let mut first_out = true;
    for c in &events {
        let is_comment_event = c.is_named() && c.kind().ends_with("comment");
        // 若该事件是已合并的注释（前有单行 case_item），跳过
        if is_comment_event && text_idx < out_texts.len() {
            let prev_event = events
                .iter()
                .rfind(|e| e.byte_range().end <= c.byte_range().start);
            let inline = prev_event
                .map(|e| {
                    !f.ws(e.byte_range().end, c.byte_range().start)
                        .contains('\n')
                })
                .unwrap_or(false);
            if inline && text_idx > 0 {
                // 已合并进上一个 case_item 行
                continue;
            }
        }
        if !first_out {
            let ws = prev_end
                .map(|pe| f.ws(pe, c.byte_range().start))
                .unwrap_or("");
            let blanks = crate::formatter::count_blank_lines(ws);
            if blanks > 0 {
                docs.push(Doc::BlankLines(blanks));
            } else if has_newline(ws) {
                docs.push(Doc::Newline);
            } else {
                docs.push(Doc::Space);
            }
        }
        if let Some(text) = out_texts.get(text_idx).cloned().flatten() {
            docs.push(Doc::text(text));
            text_idx += 1;
        }
        first_out = false;
        prev_end = Some(c.byte_range().end);
    }
    while let Some(text) = out_texts.get(text_idx).cloned().flatten() {
        docs.push(Doc::text(text));
        text_idx += 1;
    }
    docs.push(Doc::Dedent);
    // endcase 前保留空行
    let endcase_node = items.iter().find(|c| c.kind() == "endcase").copied();
    let blank_before_endcase = events
        .last()
        .and_then(|e| endcase_node.map(|en| f.ws(e.byte_range().end, en.byte_range().start)))
        .map(|ws| crate::formatter::count_blank_lines(ws) > 0)
        .unwrap_or(false);
    if blank_before_endcase {
        let blanks = events
            .last()
            .and_then(|e| endcase_node.map(|en| f.ws(e.byte_range().end, en.byte_range().start)))
            .map(crate::formatter::count_blank_lines)
            .unwrap_or(0);
        docs.push(Doc::BlankLines(blanks));
    } else {
        docs.push(Doc::Newline);
    }
    docs.push(Doc::text("endcase"));
    Doc::concat(docs)
}

/// case_item 的 body 风格："inline"（单语句同行）/ "seq" / "cond"。
fn case_item_body_style(f: &Formatter<'_>, node: CstNode<'_>) -> &'static str {
    let mut seen_colon = false;
    for c in node.children() {
        if c.kind() == ":" {
            seen_colon = true;
            continue;
        }
        if seen_colon && c.is_named() {
            let inner = unwrap_statement(f, c);
            return match inner.kind() {
                "seq_block" => "seq",
                "conditional_statement" => "cond",
                _ => "inline",
            };
        }
    }
    "inline"
}

/// case_item 的 body 是否为单行（单语句同行）。
/// 提取 case item 冒号前的表达式文本。
fn case_item_expr_text(f: &Formatter<'_>, node: CstNode<'_>) -> String {
    let _ = f;
    let items: Vec<CstNode<'_>> = node.children();
    let mut out = String::new();
    for c in items {
        if c.kind() == ":" {
            break;
        }
        out.push_str(c.text());
    }
    out
}

/// case_item：`expr : body`。
pub fn fmt_case_item(f: &Formatter<'_>, node: CstNode<'_>, align_width: &[usize]) -> Doc {
    let items: Vec<CstNode<'_>> = node.children();
    // 检测 body 是否 seq_block（此时冒号前只留 1 空格）
    let body_is_seq = {
        let mut seen_colon = false;
        let mut is_seq = false;
        for c in &items {
            if c.kind() == ":" {
                seen_colon = true;
                continue;
            }
            if seen_colon && c.is_named() {
                let inner = unwrap_statement(f, *c);
                if inner.kind() == "seq_block" {
                    is_seq = true;
                }
                break;
            }
        }
        is_seq
    };
    let mut expr_doc: Vec<Doc> = Vec::new();
    let mut body: Option<CstNode<'_>> = None;
    let mut body_expr = false;
    let mut colon_end: Option<usize> = None;
    let mut i = 0;
    while i < items.len() {
        let c = items[i];
        if c.kind() == ":" {
            // 冒号前补对齐空格（至少 1 个空格）
            let cur = case_item_expr_text(f, node).chars().count();
            let pad_n = if body_is_seq {
                1
            } else {
                let w = align_width.first().copied().unwrap_or(0);
                if w > cur { w - cur + 1 } else { 1 }
            };
            let mut pad = String::new();
            for _ in 0..pad_n {
                pad.push(' ');
            }
            expr_doc.push(Doc::text(pad));
            expr_doc.push(Doc::text(":"));
            colon_end = Some(c.byte_range().end);
            i += 1;
            continue;
        }
        if c.is_named()
            && c.kind() != "case_item_expression"
            && c.kind() != "expression"
            && c.kind() != "case_item_expression_list"
        {
            // 冒号后的 body
            body = Some(c);
            body_expr = true;
            break;
        }
        expr_doc.push(Doc::text(c.text()));
        i += 1;
    }
    // body 处理
    let mut docs: Vec<Doc> = expr_doc;
    if let Some(b) = body {
        let inner = unwrap_statement(f, b);
        match inner.kind() {
            "seq_block" => {
                // `:` 后的原文尾随空格（若 body 在下一行）
                if let Some(ce) = colon_end {
                    let ws = f.ws(ce, b.byte_range().start);
                    if ws.contains('\n') {
                        let trailing = f.trailing_inline(ws);
                        if !trailing.is_empty() {
                            docs.push(Doc::text(trailing.to_string()));
                        }
                    }
                }
                docs.push(Doc::Newline);
                docs.push(fmt_seq_block(f, inner));
            }
            "conditional_statement" => {
                docs.push(Doc::Newline);
                docs.push(Doc::Indent);
                docs.push(fmt_conditional(f, inner));
                docs.push(Doc::Dedent);
            }
            _ => {
                // 单语句同行，或带注释
                docs.push(Doc::Space);
                docs.push(f.fmt(b));
            }
        }
    }
    let _ = body_expr;
    Doc::concat(docs)
}

/// loop_statement（for）。
pub fn fmt_loop_statement(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();
    let mut i = 0;
    let mut body: Option<CstNode<'_>> = None;
    // for ( init ; cond ; update ) body
    let mut in_header = true;
    while i < items.len() {
        let c = items[i];
        match c.kind() {
            "for" => {
                if std::env::var("SVDBG").is_ok() {
                    eprintln!("[loop] FOR");
                }
                docs.push(Doc::text("for"));
                i += 1;
            }
            "(" => {
                if std::env::var("SVDBG").is_ok() {
                    eprintln!("[loop] OPEN");
                }
                docs.push(Doc::text("("));
                i += 1;
            }
            ")" => {
                docs.push(Doc::text(")"));
                i += 1;
                in_header = false;
            }
            "statement_or_null" | "generate_block" => {
                body = Some(c);
                break;
            }
            ";" => {
                docs.push(Doc::text(";"));
                // 分号后空格（AfterSemicolon）
                if in_header && f.cfg.space.after_semicolon {
                    docs.push(Doc::Space);
                }
                i += 1;
            }
            _ if in_header => {
                docs.push(fmt_expr(
                    f,
                    c,
                    &crate::formatter::expressions::ExprCtx::default(),
                ));
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    if let Some(b) = body {
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
    if std::env::var("SVDBG").is_ok() {
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
    if std::env::var("SVDBG").is_ok() {
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

fn has_colon_label(children: &[CstNode<'_>]) -> bool {
    children
        .windows(2)
        .any(|w| w[0].kind() == ":" && w[1].kind() == "simple_identifier")
}
