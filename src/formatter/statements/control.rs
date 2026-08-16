//! if/else 链、语句体、断言语句布局。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::formatter::expressions::fmt_expr;
use crate::parser::CstNode;

use super::fmt_seq_block;
use super::render_doc;
use super::unwrap_statement;

/// conditional_statement：if/else if/else。
pub(crate) fn fmt_conditional(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let items: Vec<CstNode<'_>> = node.children();

    // —— 连续单行注释对齐预计算 ——
    // 规则：链内"连续"（无空行、中间无非注释单语句）的多个行内注释对齐到同一列
    // （相对链基准的最长内容宽度 + 2）；孤立注释保持自然位置（2 空格）。
    // 注释点：(anchor_start, anchor_end, code_end_rel, is_body)
    //   body 单语句 → code_end_rel = indent_width + 语句宽（body 比链基准多一层缩进）
    //   else 行     → code_end_rel = 4（"else"）
    //   else if 行  → code_end_rel = "else if(cond)" 宽
    let indent_w = f.cfg.indent_width as usize;
    let mut pts: Vec<(usize, usize, usize)> = Vec::new();
    {
        let has_inline = |node: CstNode<'_>, next: &CstNode<'_>| -> bool {
            next.is_named()
                && next.kind().ends_with("comment")
                && !f
                    .ws(node.byte_range().end, next.byte_range().start)
                    .contains('\n')
        };
        for (i, c) in items.iter().enumerate() {
            match c.kind() {
                "statement_or_null" => {
                    let inner = unwrap_statement(f, *c);
                    if !matches!(inner.kind(), "seq_block" | "conditional_statement")
                        && items.get(i + 1).map(|n| has_inline(*c, n)).unwrap_or(false)
                    {
                        let w = render_doc(f, f.fmt(*c)).chars().count();
                        pts.push((c.byte_range().start, c.byte_range().end, indent_w + w));
                    }
                }
                "else" if items.get(i + 1).map(|n| has_inline(*c, n)).unwrap_or(false) => {
                    pts.push((c.byte_range().start, c.byte_range().end, 4));
                }
                "cond_predicate" => {
                    let after = items.get(i + 1);
                    let cc = if after.is_some_and(|a| a.kind() == ")") {
                        items.get(i + 2)
                    } else {
                        after
                    };
                    if cc.map(|n| has_inline(*c, n)).unwrap_or(false) {
                        let cond_w =
                            render_doc(f, crate::formatter::module::fmt_cond_expression(f, *c))
                                .chars()
                                .count();
                        let mut w = 4 + 1 + 2 + cond_w;
                        if f.cfg.space.before_control_statement_parens {
                            w += 1;
                        }
                        pts.push((c.byte_range().start, c.byte_range().end, w));
                    }
                }
                _ => {}
            }
        }
    }
    // 划分连续 run，计算每个注释点的对齐目标（相对链基准）
    let n = pts.len();
    let mut targets: Vec<Option<usize>> = vec![None; n];
    if n >= 2 {
        let mut brk = vec![false; n];
        for k in 1..n {
            // 空行 → 断组
            let ws = f.ws(pts[k - 1].1, pts[k].0);
            if crate::formatter::count_blank_lines(ws) > 0 {
                brk[k] = true;
                continue;
            }
            // 两个注释点之间的非注释单语句 → 断组
            for c in &items {
                if c.byte_range().start <= pts[k - 1].1 || c.byte_range().start >= pts[k].0 {
                    continue;
                }
                if c.is_named() && c.kind().ends_with("comment") {
                    continue;
                }
                if c.kind() == "statement_or_null" {
                    let inner = unwrap_statement(f, *c);
                    if !matches!(inner.kind(), "seq_block" | "conditional_statement") {
                        brk[k] = true;
                        break;
                    }
                }
            }
        }
        let mut i = 0;
        while i < n {
            let mut j = i;
            while j + 1 < n && !brk[j + 1] {
                j += 1;
            }
            if j > i {
                let run_max = pts[i..=j].iter().map(|p| p.2).max().unwrap_or(0);
                let target = run_max + 2;
                for t in targets[i..=j].iter_mut() {
                    *t = Some(target);
                }
            }
            i = j + 1;
        }
    }
    // 按注释点 anchor 起始字节查对齐目标
    let target_for = |start: usize| -> Option<usize> {
        pts.iter()
            .position(|p| p.0 == start)
            .and_then(|k| targets[k])
    };

    let mut is_else = false;
    let mut cond_after_else = false;
    for (i, child) in items.iter().enumerate() {
        match child.kind() {
            "if" => {
                if is_else {
                    docs.push(Doc::Space);
                }
                docs.push(Doc::text("if"));
                if f.cfg.space.before_control_statement_parens {
                    docs.push(Doc::Space);
                }
                cond_after_else = false;
            }
            "else" => {
                if f.cfg.else_on_newline || f.cfg.end_on_newline {
                    docs.push(Doc::Newline);
                } else {
                    docs.push(Doc::Space);
                }
                docs.push(Doc::text("else"));
                // 行尾注释（`else // comment`，直接跟语句时）
                if let Some(next) = items.get(i + 1)
                    && next.is_named()
                    && next.kind().ends_with("comment")
                    && !f
                        .ws(child.byte_range().end, next.byte_range().start)
                        .contains('\n')
                {
                    let pad = target_for(child.byte_range().start)
                        .map(|t| t.saturating_sub(4))
                        .unwrap_or(2);
                    for _ in 0..pad {
                        docs.push(Doc::Space);
                    }
                    docs.push(Doc::text(next.text()));
                }
                is_else = true;
                cond_after_else = false;
            }
            "cond_predicate" => {
                docs.push(crate::formatter::module::fmt_cond_expression(f, *child));
                // 行尾注释（`else if(cond) // comment`，跳过 `)`）
                let after = items.get(i + 1);
                let comment_candidate = if after.is_some_and(|a| a.kind() == ")") {
                    items.get(i + 2)
                } else {
                    after
                };
                if let Some(cmt) = comment_candidate
                    && cmt.is_named()
                    && cmt.kind().ends_with("comment")
                    && !f
                        .ws(child.byte_range().end, cmt.byte_range().start)
                        .contains('\n')
                {
                    let pad = match target_for(child.byte_range().start) {
                        Some(t) => {
                            let cond_w = render_doc(
                                f,
                                crate::formatter::module::fmt_cond_expression(f, *child),
                            )
                            .chars()
                            .count();
                            let mut w = 4 + 1 + 2 + cond_w;
                            if f.cfg.space.before_control_statement_parens {
                                w += 1;
                            }
                            t.saturating_sub(w)
                        }
                        None => 2,
                    };
                    for _ in 0..pad {
                        docs.push(Doc::Space);
                    }
                    docs.push(Doc::text(cmt.text()));
                }
                if f.cfg.begin_end_on_newline {
                    docs.push(Doc::Newline);
                } else {
                    docs.push(Doc::Space);
                }
                cond_after_else = true;
            }
            "statement_or_null" => {
                if is_else && !cond_after_else {
                    if f.cfg.else_on_newline || f.cfg.end_on_newline {
                        docs.push(Doc::Newline);
                    } else {
                        docs.push(Doc::Space);
                    }
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
                        // 语句内行尾注释：默认对齐到 semi+3；连续 run 时对齐到目标列
                        let base_indent = f.cfg.comment_column as usize;
                        let bms = match target_for(child.byte_range().start) {
                            Some(t) => (t - indent_w).saturating_sub(3),
                            None => semi_rel,
                        };
                        let padded = crate::formatter::module::pad_comment_pub(
                            f,
                            &text,
                            cmt.text(),
                            base_indent,
                            bms,
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

/// 语句体：seq_block 换行 begin；单语句缩进；if/else 链同样缩进一层。
pub(crate) fn fmt_body(f: &Formatter<'_>, node: CstNode<'_>, _depth: usize) -> Doc {
    let inner = unwrap_statement(f, node);
    match inner.kind() {
        "seq_block" => fmt_seq_block(f, inner),
        "conditional_statement" => {
            // if/else 链作为语句体时缩进一层（如 always 内无 begin/end 的 if 链）
            Doc::concat(vec![Doc::Indent, fmt_conditional(f, inner), Doc::Dedent])
        }
        _ => {
            // 单语句：用包装节点格式化（补齐 `;`）
            let body_doc = f.fmt(node);
            Doc::concat(vec![Doc::Indent, body_doc, Doc::Dedent])
        }
    }
}

/// `simple_immediate_assert_statement`：`assert(cond) [else statement]`。
///
/// 结构化输出（而非 raw 原文），避免原文缩进与外部 `Indent` 叠加，导致
/// 重复格式化时 else 分支缩进逐轮累积、破坏幂等性（见 examples/bsg.sv 的
/// `assert(...) else $error(...)`）。输出形态：
///
/// ```text
/// assert(cond) else
///     $error(...);
/// ```
pub(crate) fn fmt_assert(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    for child in node.children_iter() {
        match child.kind() {
            "assert" => docs.push(Doc::text("assert")),
            "(" => docs.push(Doc::text("(")),
            ")" => docs.push(Doc::text(")")),
            ";" => docs.push(Doc::text(";")),
            "expression" | "constant_expression" => {
                docs.push(fmt_expr(
                    f,
                    child,
                    &crate::formatter::expressions::ExprCtx::default(),
                ));
            }
            "action_block" => {
                // action_block：`[else statement]` 或 `[statement]`
                let mut sub_body: Option<CstNode<'_>> = None;
                let mut block_else = false;
                for sub in child.children() {
                    if sub.kind() == "else" {
                        block_else = true;
                    } else if sub.is_named() {
                        sub_body = Some(sub);
                    }
                }
                if block_else {
                    docs.push(Doc::Space);
                    docs.push(Doc::text("else"));
                }
                if let Some(b) = sub_body {
                    let inner = unwrap_statement(f, b);
                    if inner.kind() == "seq_block" {
                        docs.push(Doc::Space);
                        docs.push(fmt_seq_block(f, inner));
                    } else if block_else {
                        docs.push(Doc::Newline);
                        docs.push(Doc::Indent);
                        docs.push(f.fmt(b));
                        docs.push(Doc::Dedent);
                    } else {
                        docs.push(Doc::Space);
                        docs.push(f.fmt(b));
                    }
                }
            }
            _ => {
                if child.is_named() {
                    docs.push(f.fmt(child));
                } else {
                    docs.push(Doc::text(child.text()));
                }
            }
        }
    }
    Doc::concat(docs)
}
