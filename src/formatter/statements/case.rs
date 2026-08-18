//! case 语句布局。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::formatter::expressions::fmt_expr;
use crate::formatter::tokens::has_newline;
use crate::parser::CstNode;

use super::control::fmt_conditional;
use super::fmt_seq_block;
use super::render_doc;
use super::unwrap_statement;

/// case_statement。
pub(crate) fn fmt_case_statement(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();
    // case 关键字 + 表达式
    let mut i = 0;
    while i < items.len() {
        let c = items[i];
        if c.kind() == "case_keyword" || c.kind() == "casez_keyword" || c.kind() == "casex_keyword"
        {
            let text = match f.cfg.reformat_case {
                crate::config::ReformatCase::None => c.text(),
                crate::config::ReformatCase::Casez => "casez",
                crate::config::ReformatCase::Casex => "casex",
            };
            docs.push(Doc::text(text));
            if f.cfg.space.before_control_statement_parens {
                docs.push(Doc::Space);
            }
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
    // 预渲染所有 case_item：行尾注释的公共对齐列 = 所在"连续 run"内最长行的 `;`。
    // 规则：只有"相邻（无空行间隔）的带行内注释单行分支"组成一组，组内注释对齐。
    let pre_texts: Vec<String> = items_only
        .iter()
        .map(|c| render_doc(f, fmt_case_item(f, *c, &[max_w])))
        .collect();
    let n = pre_texts.len();
    // 每个 item 是否为"带行内注释的单行分支"
    let mut commented = vec![false; n];
    for (idx, t) in pre_texts.iter().enumerate() {
        if t.contains('\n') {
            continue;
        }
        let ci = items_only[idx];
        commented[idx] = events
            .iter()
            .find(|e| e.byte_range().start >= ci.byte_range().end)
            .map(|next| {
                next.is_named()
                    && next.kind().ends_with("comment")
                    && !f
                        .ws(ci.byte_range().end, next.byte_range().start)
                        .contains('\n')
            })
            .unwrap_or(false);
    }
    // 相邻判断：两个 case_item 之间的空白无空行
    let adjacent = |j: usize| -> bool {
        let a = items_only[j];
        let b = items_only[j + 1];
        let ws = f.ws(a.byte_range().end, b.byte_range().start);
        crate::formatter::count_blank_lines(ws) == 0
    };
    // 划分连续 run，run 内所有 item 的对齐列 = run 内最长 `;`
    let mut align: Vec<usize> = vec![0; n];
    let mut i = 0usize;
    while i < n {
        if !commented[i] {
            i += 1;
            continue;
        }
        let mut j = i;
        let mut run_max = pre_texts[i].rfind(';').unwrap_or(0);
        while j + 1 < n && commented[j + 1] && adjacent(j) {
            j += 1;
            let s = pre_texts[j].rfind(';').unwrap_or(0);
            if s > run_max {
                run_max = s;
            }
        }
        for a in align[i..=j].iter_mut() {
            *a = run_max;
        }
        i = j + 1;
    }
    docs.push(Doc::Newline);
    for _ in 0..f.cfg.case_indent_level {
        docs.push(Doc::Indent);
    }
    // 重新构建行：注释同行则合并（对齐到所在 run 的最长 `;` + 3）
    let mut out_texts: Vec<Option<String>> = Vec::new();
    let mut current: Option<(String, usize)> = None; // (单行文本, items_only 索引)
    let mut pi = 0usize;
    for c in &events {
        if c.kind() == "case_item" {
            if let Some((l, _)) = current.take() {
                out_texts.push(Some(l));
            }
            let doc = fmt_case_item(f, *c, &[max_w]);
            let text = render_doc(f, doc);
            let idx = pi;
            pi += 1;
            if text.contains('\n') {
                out_texts.push(Some(text));
            } else {
                current = Some((text, idx));
            }
        } else {
            // 注释
            if let Some((l, idx)) = current.take() {
                let cmt_text = c.text();
                let prev_event = events
                    .iter()
                    .rfind(|e| e.byte_range().end <= c.byte_range().start);
                let ws = prev_event
                    .map(|e| f.ws(e.byte_range().end, c.byte_range().start))
                    .unwrap_or("");
                if !ws.contains('\n') {
                    let padded = crate::formatter::module::pad_comment_pub(
                        f,
                        &l,
                        cmt_text,
                        f.cfg.comment_column as usize,
                        align[idx],
                    );
                    out_texts.push(Some(padded));
                } else {
                    out_texts.push(Some(l));
                    out_texts.push(Some(f.comment_text(*c)));
                }
            } else {
                out_texts.push(Some(f.comment_text(*c)));
            }
        }
    }
    if let Some((l, _)) = current {
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

/// case_generate_construct（generate 内 case）。
pub(crate) fn fmt_case_generate_construct(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();

    // case 关键字 + 表达式头
    let mut i = 0usize;
    while i < items.len() {
        let c = items[i];
        match c.kind() {
            "case" | "casez" | "casex" => {
                docs.push(Doc::text(c.text()));
                if f.cfg.space.before_control_statement_parens {
                    docs.push(Doc::Space);
                }
                i += 1;
            }
            "(" => {
                docs.push(Doc::text("("));
                i += 1;
            }
            ")" => {
                docs.push(Doc::text(")"));
                i += 1;
            }
            "case_generate_item" | "endcase" => break,
            _ if c.is_named() => {
                docs.push(fmt_expr(
                    f,
                    c,
                    &crate::formatter::expressions::ExprCtx::default(),
                ));
                i += 1;
            }
            _ => i += 1,
        }
    }

    // case item
    let events: Vec<CstNode<'_>> = items
        .iter()
        .copied()
        .skip(i)
        .take_while(|c| c.kind() != "endcase")
        .filter(|c| {
            c.kind() == "case_generate_item" || (c.is_named() && c.kind().ends_with("comment"))
        })
        .collect();

    docs.push(Doc::Newline);
    for _ in 0..f.cfg.case_indent_level {
        docs.push(Doc::Indent);
    }

    let mut prev_end: Option<usize> = None;
    let mut first = true;
    for c in &events {
        if !first {
            let ws = prev_end
                .map(|pe| f.ws(pe, c.byte_range().start))
                .unwrap_or("");
            let blanks = crate::formatter::count_blank_lines(ws);
            if blanks > 0 {
                docs.push(Doc::BlankLines(blanks));
            } else {
                docs.push(Doc::Newline);
            }
        }
        if c.kind() == "case_generate_item" {
            docs.push(fmt_case_generate_item(f, *c));
        } else {
            docs.push(Doc::text(f.comment_text(*c)));
        }
        prev_end = Some(c.byte_range().end);
        first = false;
    }

    docs.push(Doc::Dedent);
    docs.push(Doc::Newline);
    docs.push(Doc::text("endcase"));
    Doc::concat(docs)
}

fn fmt_case_generate_item(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();

    let mut seen_colon = false;
    let mut body: Option<CstNode<'_>> = None;
    let mut pending_space = false;

    for c in &items {
        if c.kind() == ":" {
            docs.push(Doc::text(":"));
            seen_colon = true;
            continue;
        }
        if !seen_colon {
            if c.is_named() {
                if pending_space {
                    docs.push(Doc::Space);
                    pending_space = false;
                }
                docs.push(fmt_expr(
                    f,
                    *c,
                    &crate::formatter::expressions::ExprCtx::default(),
                ));
            } else if c.kind() == "," {
                docs.push(Doc::text(","));
                pending_space = true;
            }
        } else if c.is_named() {
            body = Some(*c);
            break;
        }
    }

    if let Some(b) = body {
        docs.push(Doc::Newline);
        docs.push(f.fmt(b));
    }
    Doc::concat(docs)
}

/// case_item 的 body 风格："inline"（单语句同行）/ "seq" / "cond"。
fn case_item_body_style(f: &Formatter<'_>, node: CstNode<'_>) -> &'static str {
    let mut seen_colon = false;
    for c in node.children_iter() {
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
pub(crate) fn fmt_case_item(f: &Formatter<'_>, node: CstNode<'_>, align_width: &[usize]) -> Doc {
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
            } else if f.cfg.align_case_items {
                let w = align_width.first().copied().unwrap_or(0);
                if w > cur { w - cur + 1 } else { 1 }
            } else {
                1
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
