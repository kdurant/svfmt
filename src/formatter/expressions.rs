//! 表达式与 token 间隔规则。
//!
//! 负责 token 之间空白（空格/无/保留原文），以及表达式节点的结构化排版。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::formatter::tokens::{Token, has_newline, leaf_tokens};
use crate::parser::CstNode;

/// token 间隔选项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sep {
    /// 无空白。
    None,
    /// 一个空格。
    Space,
    /// 保留原文行内空白。
    Keep,
}

/// 表达式格式化上下文。
#[derive(Debug, Clone, Copy, Default)]
pub struct ExprCtx {
    /// 处于拼接（concatenation）内部：逗号后不加空格。
    pub in_concat: bool,
    /// 处于 packed dimension / constant range 内部：运算符与冒号不加空格。
    pub in_packed_dim: bool,
}

/// 判断节点是否属于表达式/值节点，需要递归格式化。
pub fn is_value_kind(kind: &str) -> bool {
    matches!(
        kind,
        "expression"
            | "constant_expression"
            | "constant_param_expression"
            | "constant_mintypmax_expression"
            | "mintypmax_expression"
            | "primary"
            | "constant_primary"
            | "primary_literal"
            | "integral_number"
            | "real_number"
            | "unsigned_number"
            | "signed_number"
            | "decimal_number"
            | "hex_number"
            | "octal_number"
            | "binary_number"
            | "time_literal"
            | "string"
            | "string_literal"
            | "identifier"
            | "simple_identifier"
            | "escaped_identifier"
            | "hierarchical_identifier"
            | "hierarchical_array_identifier"
            | "indexed_identifier"
            | "bit_select"
            | "indexed_range_select"
            | "constant_bit_select"
            | "concatenation"
            | "streaming_concatenation"
            | "parenthesized_expression"
            | "unary_expression"
            | "binary_expression"
            | "ternary_expression"
            | "conditional_expression"
            | "assignment_expression"
            | "implicit_class_handle"
            | "function_call"
            | "system_function_call"
            | "method_call"
            | "call"
            | "class_new"
            | "list_of_arguments"
            | "actual_argument"
            | "constant_range"
            | "range_expression"
            | "packed_dimension"
            | "unpacked_dimension"
            | "select"
            | "expression_list"
            | "event_expression"
            | "clocking_event"
            | "event_control"
            | "delay_control"
            | "delay_value"
            | "text_macro_usage"
            | "list_of_variable_identifiers"
            | "list_of_net_identifiers"
            | "variable_lvalue"
            | "net_lvalue"
            | "list_of_variable_assignments"
            | "variable_assignment"
            | "constant_expression_list"
            | "case_item_expression"
            | "let_expression"
            | "tagged_union_expression"
            | "assignment_pattern"
            | "casting_type"
            | "signing"
            | "integer_atom_type"
            | "integer_vector_type"
            | "integer_type"
            | "data_type"
            | "data_type_or_implicit"
            | "simple_type"
            | "net_type"
            | "net_port_type"
            | "variable_port_type"
            | "type_reference"
            | "element_select"
            | "hierarchical_branch"
            | "base_expression"
            | "array_manipulation_call"
            | "struct_union_member"
            | "enum_name_declaration"
            | "named_parameter_assignment"
            | "ordered_parameter_assignment"
            | "named_port_connection"
            | "ordered_port_connection"
            | "parameter_assignment"
            | "port_connection"
            | "list_of_param_assignments"
            | "param_assignment"
            | "list_of_port_connections"
            | "list_of_parameter_assignments"
            | "struct_union"
            | "non_default_type"
            | "constant_part_select_range"
    )
}

/// 是否为控制语句关键字。
fn is_control_kw(kind: &str) -> bool {
    matches!(
        kind,
        "if" | "for" | "while" | "case" | "casez" | "casex" | "return" | "assert"
    )
}

/// 是否为二元运算符。
pub fn is_binary_op(kind: &str) -> bool {
    matches!(
        kind,
        "+" | "-"
            | "*"
            | "/"
            | "%"
            | "=="
            | "!="
            | "==="
            | "!=="
            | "&&"
            | "||"
            | "&"
            | "|"
            | "^"
            | "^~"
            | "~^"
            | "<"
            | ">"
            | "<="
            | ">="
            | "<<"
            | ">>"
            | "<<<"
            | ">>>"
            | "**"
    )
}

/// 是否为赋值运算符。
pub fn is_assignment_op(kind: &str) -> bool {
    matches!(
        kind,
        "=" | "<="
            | ">="
            | "+="
            | "-="
            | "*="
            | "/="
            | "%="
            | "&="
            | "|="
            | "^="
            | "<<="
            | ">>="
            | "<<<="
            | ">>>="
            | ":="
            | "::="
    )
}

/// 是否为一元运算符。
fn is_unary_op(kind: &str) -> bool {
    matches!(kind, "!" | "~" | "~&" | "~|" | "~^" | "^^" | "++" | "--")
}

/// 计算 token 之间的间隔。
pub fn token_sep(f: &Formatter<'_>, prev: Option<&Token>, cur: &Token, ctx: &ExprCtx) -> Sep {
    let cfg = &f.cfg;
    if let Some(prev) = prev {
        // 宏引用反引号：反引号与宏名之间无空格（`MACRO）。
        // 反引号作为 prev 时其后紧跟宏名 → 无空格；
        // 反引号作为 cur 时其前（如二元运算符 `==` 后）需空格 → Space。
        if cur.kind == "`" {
            return Sep::Space;
        }
        if prev.kind == "`" {
            return Sep::None;
        }
        // 点
        if prev.kind == "." || cur.kind == "." {
            return Sep::None;
        }
        // 作用域解析 `::`：无空格（`bsp::DEVICE_MAC`）
        if prev.kind == "::" || cur.kind == "::" {
            return Sep::None;
        }
        // 括号
        if prev.kind == "(" || cur.kind == ")" {
            return if cfg.space.inside_parens {
                Sep::Space
            } else {
                Sep::None
            };
        }
        // 方括号（packed/unpacked dimension 内部无空格）
        if prev.kind == "[" || cur.kind == "]" {
            return Sep::None;
        }
        // 逗号
        if cur.kind == "," {
            return Sep::None;
        }
        if prev.kind == "," {
            if ctx.in_concat {
                return Sep::None;
            }
            return if cfg.space.after_comma {
                Sep::Space
            } else {
                Sep::None
            };
        }
        // 分号
        if cur.kind == ";" {
            return Sep::None;
        }
        if prev.kind == ";" {
            return if cfg.space.after_semicolon {
                Sep::Space
            } else {
                Sep::None
            };
        }
        // 冒号
        if cur.kind == ":" {
            // constant_range 内 `[a:b]` 无空格
            if ctx.in_packed_dim {
                return Sep::None;
            }
            return if cfg.space.before_colon {
                Sep::Space
            } else {
                Sep::None
            };
        }
        if prev.kind == ":" {
            if ctx.in_packed_dim {
                return Sep::None;
            }
            // 三元表达式 `a : (b)` 冒号后括号不空格
            if cur.kind == "(" {
                return Sep::None;
            }
            return if cfg.space.after_colon {
                Sep::Space
            } else {
                Sep::None
            };
        }
        // 问号（ternary）
        if prev.kind == "?" || cur.kind == "?" {
            return Sep::Space;
        }
        // 赋值运算符
        if is_assignment_op(cur.kind) || is_assignment_op(prev.kind) {
            if cur.kind == "=" && is_unary_prefix(prev.kind) {
                return Sep::None;
            }
            return if cfg.space.around_assignment {
                Sep::Space
            } else {
                Sep::None
            };
        }
        // 位选/范围 `+:` `-:` 无空格
        if cur.kind == "+:" || cur.kind == "-:" || prev.kind == "+:" || prev.kind == "-:" {
            return Sep::None;
        }
        // 加减：可能是二元或一元
        if cur.kind == "+" || cur.kind == "-" {
            // range 内部（如 [a-1:0]）运算符无空格
            if ctx.in_packed_dim {
                return Sep::None;
            }
            if is_unary_prefix(prev.kind) {
                // 一元 +/-
                return if cfg.space.after_unary_operators {
                    Sep::Space
                } else {
                    Sep::None
                };
            }
            return if cfg.space.around_binary_operator {
                Sep::Space
            } else {
                Sep::None
            };
        }
        // 其他一元运算符（! ~ 等）位于操作数前；若其前是二元运算符，则前有空格
        if is_unary_op(cur.kind) {
            if is_binary_op(prev.kind) {
                return if cfg.space.around_binary_operator {
                    Sep::Space
                } else {
                    Sep::None
                };
            }
            return if cfg.space.after_unary_operators {
                Sep::Space
            } else {
                Sep::None
            };
        }
        // 一元运算符后（`~a` 等；加减一元由 fmt_unary 处理）
        if is_unary_op(prev.kind) {
            return if cfg.space.after_unary_operators {
                Sep::Space
            } else {
                Sep::None
            };
        }
        // 二元运算符
        if is_binary_op(cur.kind) || is_binary_op(prev.kind) {
            if f.svdbg() && (cur.kind == "*" || prev.kind == "*") {
                eprintln!(
                    "[sep*] cur={} prev={} packed={}",
                    cur.kind, prev.kind, ctx.in_packed_dim
                );
            }
            if ctx.in_packed_dim {
                return Sep::None;
            }
            return if cfg.space.around_binary_operator {
                Sep::Space
            } else {
                Sep::None
            };
        }
        // `@` 后
        if prev.kind == "@" {
            return if cfg.space.after_at {
                Sep::Space
            } else {
                Sep::None
            };
        }
        // `#` 后（参数列表）
        if prev.kind == "#" || cur.kind == "#" {
            return Sep::None;
        }
        // `(` 前：控制语句/函数调用/索引，默认无空格
        if cur.kind == "(" {
            if cfg.space.before_control_statement_parens && is_control_kw(prev.kind) {
                return Sep::Space;
            }
            if cfg.space.before_parens_in_function_call {
                return Sep::Space;
            }
            return Sep::None;
        }
        // `[` 前（索引）：无空格
        if cur.kind == "[" {
            return Sep::None;
        }
    }
    Sep::Keep
}

/// `-`/`+` 前一个 token 是否表示一元上下文。
fn is_unary_prefix(kind: &str) -> bool {
    is_binary_op(kind)
        || is_assignment_op(kind)
        || is_unary_op(kind)
        || matches!(kind, "(" | "[" | "?" | ":" | "," | ";" | "@" | "{" | "}")
        || matches!(kind, "if" | "for" | "while" | "return" | "case")
}

/// 通用 token 序列格式化：按间隔规则输出，未匹配规则时保留原文行内空白。
pub fn fmt_default(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let toks = leaf_tokens(node);
    let mut prev: Option<Token> = None;
    let ctx = ExprCtx::default();
    for tok in &toks {
        if let Some(p) = &prev {
            let sep = token_sep(f, Some(p), tok, &ctx);
            // 注意：不在 fallback（token 序列）路径发射 SoftLine。
            // fmt_default 用于连续赋值 / net_assignment 等 fallback 上下文，
            // 内部 SoftLine 断行会依赖原文空白（Sep::Keep），在 generate 块 /
            // ERROR 区域等脆弱位置会破坏解析结构、导致幂等性漂移
            // （见 examples/hdmi.sv）。断行只由结构化路径
            // （fmt_children_expr / fmt_assignment / fmt_call）负责。
            apply_sep(f, &mut docs, p, tok, sep);
        }
        docs.push(Doc::text(tok.text));
        prev = Some(*tok);
    }
    if docs.is_empty() {
        Doc::Nil
    } else {
        Doc::concat(docs)
    }
}

fn apply_sep(f: &Formatter<'_>, docs: &mut Vec<Doc>, prev: &Token, cur: &Token, sep: Sep) {
    match sep {
        Sep::None => {}
        Sep::Space => docs.push(Doc::Space),
        Sep::Keep => {
            let ws = f.ws(prev.byte_range.1, cur.byte_range.0);
            let inline = f.leading_inline(ws);
            if !inline.is_empty() {
                docs.push(Doc::text(inline.to_string()));
            }
        }
    }
}

/// 递归格式化一个值/表达式节点。
pub fn fmt_expr(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    let kind = node.kind();
    if f.svdbg() {
        eprintln!(
            "[expr] kind={} text={:?}",
            kind,
            node.text().split('\n').next()
        );
    }
    match kind {
        "binary_expression" => fmt_binary(f, node, ctx),
        "unary_expression" => fmt_unary(f, node, ctx),
        "ternary_expression" | "conditional_expression" => fmt_ternary(f, node, ctx),
        "concatenation" | "streaming_concatenation" => fmt_concat(f, node, ctx),
        "parenthesized_expression" => {
            let mut docs = vec![Doc::text("(")];
            let mut inner_ctx = *ctx;
            inner_ctx.in_packed_dim = false;
            for child in node.children_iter() {
                docs.push(fmt_child_or_token(f, child, &inner_ctx));
            }
            docs.push(Doc::text(")"));
            Doc::concat(docs)
        }
        "constant_range" | "range_expression" => fmt_range(f, node, ctx),
        "select"
        | "bit_select"
        | "indexed_range_select"
        | "constant_part_select_range"
        | "indexed_range" => {
            let inner = ExprCtx {
                in_packed_dim: true,
                ..*ctx
            };
            fmt_children_expr(f, node, &inner)
        }
        "packed_dimension" | "unpacked_dimension" => fmt_dimension(f, node, ctx),
        "function_call"
        | "system_function_call"
        | "call"
        | "system_tf_call"
        | "tf_call"
        | "subroutine_call"
        | "subroutine_call_statement"
        | "class_new" => fmt_call(f, node, ctx),
        "assignment_expression"
        | "blocking_assignment"
        | "nonblocking_assignment"
        | "operator_assignment" => fmt_assignment(f, node, ctx),
        _ if is_container_expr(kind) => fmt_children_expr(f, node, ctx),
        _ => {
            // 叶子/简单节点：按 token 序列 + 间隔规则
            fmt_default(f, node)
        }
    }
}

/// 是否为需要递归子表达式的容器节点。
fn is_container_expr(kind: &str) -> bool {
    matches!(
        kind,
        "expression"
            | "constant_expression"
            | "constant_param_expression"
            | "primary"
            | "constant_primary"
            | "event_expression"
            | "expression_list"
            | "constant_expression_list"
            | "list_of_arguments"
            | "case_item_expression"
            | "case_item_expression_list"
            | "select"
            | "bit_select"
            | "constant_bit_select"
            | "indexed_range_select"
            | "constant_part_select_range"
            | "hierarchical_identifier"
            | "indexed_identifier"
    )
}

/// 递归遍历子节点：named 递归表达式，anonymous 用间隔规则。
fn fmt_children_expr(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    if f.svdbg() && node.kind() == "expression" {
        eprintln!(
            "[childexpr] text={:?} ctx.packed={}",
            node.text(),
            ctx.in_packed_dim
        );
    }
    let mut docs: Vec<Doc> = Vec::new();
    let mut prev: Option<Token> = None;
    for child in node.children_iter() {
        if child.is_named() {
            let d = fmt_expr(f, child, ctx);
            if let Some(p) = prev {
                // 子节点前可能需要间隔（如 list_of_arguments 内的逗号）
                if let Some(tok) = first_token_of(child) {
                    let ws = f.ws(p.byte_range.1, tok.byte_range.0);
                    // 注释相关换行：保留原文换行（多行调用/表达式中的注释
                    // 是行导向的，折叠会破坏注释与代码的对应关系）
                    if has_newline(ws) && (p.is_comment || child.kind().ends_with("comment")) {
                        docs.push(Doc::Newline);
                    } else if child.kind().ends_with("comment") {
                        // 注释：保留原文对齐空白
                        apply_sep(f, &mut docs, &p, &tok, Sep::Keep);
                    } else {
                        let sep = token_sep(f, Some(&p), &tok, ctx);
                        // 逗号后：超宽时断行（column_limit）
                        if p.kind == "," && sep == Sep::Space {
                            docs.push(Doc::SoftLine);
                        } else {
                            apply_sep(f, &mut docs, &p, &tok, sep);
                        }
                    }
                }
            }
            docs.push(d);
        } else {
            let tok = to_token(f, child);
            if let Some(p) = prev {
                let ws = f.ws(p.byte_range.1, tok.byte_range.0);
                // 注释后换行：保留（多行调用）
                if has_newline(ws) && p.is_comment {
                    docs.push(Doc::Newline);
                } else {
                    let sep = token_sep(f, Some(&p), &tok, ctx);
                    // fallback（token 序列）路径不发射 SoftLine，避免破坏语句结构。
                    apply_sep(f, &mut docs, &p, &tok, sep);
                }
            }
            docs.push(Doc::text(tok.text));
            prev = Some(tok);
        }
        if let Some(t) = last_token_of(child) {
            // 一元运算符后：after_unary_operators 决定是否保留空格上下文
            if child.kind() == "unary_operator" {
                if f.cfg.space.after_unary_operators {
                    prev = Some(t);
                } else {
                    prev = None;
                }
            } else {
                prev = Some(t);
            }
        }
    }
    Doc::concat(docs)
}

fn first_token_of<'a>(node: CstNode<'a>) -> Option<Token<'a>> {
    leaf_tokens(node).first().copied()
}

fn last_token_of<'a>(node: CstNode<'a>) -> Option<Token<'a>> {
    leaf_tokens(node).last().copied()
}

fn fmt_child_or_token(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    if node.is_named() {
        fmt_expr(f, node, ctx)
    } else {
        Doc::text(node.text())
    }
}

fn fmt_binary(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let mut prev: Option<Token> = None;
    for child in node.children_iter() {
        if child.is_named() {
            let d = fmt_expr(f, child, ctx);
            docs.push(d);
            if let Some(t) = last_token_of(child) {
                prev = Some(t);
            }
        } else {
            let tok = to_token(f, child);
            if let Some(p) = prev {
                let sep = token_sep(f, Some(&p), &tok, ctx);
                // 二元运算符前：超宽时断行（column_limit）
                if is_binary_op(tok.kind) && sep == Sep::Space {
                    docs.push(Doc::SoftLine);
                } else {
                    apply_sep(f, &mut docs, &p, &tok, sep);
                }
            }
            docs.push(Doc::text(tok.text));
            prev = Some(tok);
        }
    }
    Doc::concat(docs)
}

fn to_token<'a>(_f: &Formatter<'a>, node: CstNode<'a>) -> Token<'a> {
    let (s, e) = {
        let r = node.byte_range();
        (r.start, r.end)
    };
    Token {
        byte_range: (s, e),
        text: node.text(),
        kind: node.kind(),
        is_comment: false,
    }
}

fn fmt_unary(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    for child in node.children_iter() {
        if child.is_named() {
            docs.push(fmt_expr(f, child, ctx));
        } else {
            docs.push(Doc::text(child.text()));
            // 一元运算符后（默认无空格）
            if f.cfg.space.after_unary_operators {
                docs.push(Doc::Space);
            }
        }
    }
    Doc::concat(docs)
}

fn fmt_ternary(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let mut prev: Option<Token> = None;
    for child in node.children_iter() {
        if child.is_named() {
            if let Some(p) = prev
                && let Some(tok) = first_token_of(child)
            {
                let sep = token_sep(f, Some(&p), &tok, ctx);
                apply_sep(f, &mut docs, &p, &tok, sep);
            }
            docs.push(fmt_expr(f, child, ctx));
            if let Some(t) = last_token_of(child) {
                prev = Some(t);
            }
        } else {
            let tok = to_token(f, child);
            if let Some(p) = prev {
                let sep = token_sep(f, Some(&p), &tok, ctx);
                apply_sep(f, &mut docs, &p, &tok, sep);
            }
            docs.push(Doc::text(tok.text));
            prev = Some(tok);
        }
    }
    Doc::concat(docs)
}

fn fmt_concat(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    if f.svdbg() {
        eprintln!(
            "[concat] children kinds: {:?}",
            node.children().iter().map(|c| c.kind()).collect::<Vec<_>>()
        );
    }
    let mut inner = *ctx;
    inner.in_concat = true;
    inner.in_packed_dim = false;
    let mut docs: Vec<Doc> = vec![Doc::text("{")];
    let mut prev: Option<Token> = None;
    for child in node.children_iter() {
        if child.kind() == "{" || child.kind() == "}" {
            continue;
        }
        if child.is_named() {
            let d = fmt_expr(f, child, &inner);
            if let Some(p) = prev
                && let Some(tok) = first_token_of(child)
            {
                let sep = if p.kind == "," {
                    // 逗号后：下一项是括号表达式则无空格，否则加空格
                    if tok.kind == "(" {
                        Sep::None
                    } else {
                        Sep::Space
                    }
                } else {
                    token_sep(f, Some(&p), &tok, &inner)
                };
                // 逗号后：超宽时断行（column_limit）
                if p.kind == "," && sep == Sep::Space {
                    docs.push(Doc::SoftLine);
                } else {
                    apply_sep(f, &mut docs, &p, &tok, sep);
                }
            }
            docs.push(d);
            if let Some(t) = last_token_of(child) {
                prev = Some(t);
            }
        } else {
            let tok = to_token(f, child);
            if let Some(p) = prev {
                let sep = token_sep(f, Some(&p), &tok, &inner);
                if f.svdbg() {
                    eprintln!(
                        "[concat] prev={:?} cur={:?} sep={:?} in_concat={}",
                        p.text, tok.text, sep, inner.in_concat
                    );
                }
                apply_sep(f, &mut docs, &p, &tok, sep);
            }
            docs.push(Doc::text(tok.text));
            prev = Some(tok);
        }
    }
    docs.push(Doc::text("}"));
    // 拼接用 Fill：超宽时贪心逐项断行（能放下就继续），而非全有或全无。
    Doc::Fill(docs)
}

fn fmt_range(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    if f.svdbg() {
        eprintln!("[range] text={:?} ctx={:?}", node.text(), ctx.in_packed_dim);
    }
    // constant_range 内部无空格（如 [DATA_WIDTH-1:0]）
    let mut inner = *ctx;
    inner.in_packed_dim = true;
    let mut docs: Vec<Doc> = Vec::new();
    let mut prev: Option<Token> = None;
    for child in node.children_iter() {
        if child.is_named() {
            docs.push(fmt_expr(f, child, &inner));
            if let Some(t) = last_token_of(child) {
                prev = Some(t);
            }
        } else {
            let tok = to_token(f, child);
            if let Some(p) = prev {
                let sep = token_sep(f, Some(&p), &tok, &inner);
                apply_sep(f, &mut docs, &p, &tok, sep);
            }
            docs.push(Doc::text(tok.text));
            prev = Some(tok);
        }
    }
    Doc::concat(docs)
}

fn fmt_dimension(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    let inner = ExprCtx {
        in_packed_dim: true,
        ..*ctx
    };
    let mut docs: Vec<Doc> = vec![Doc::text("[")];
    let mut prev: Option<Token> = None;
    for child in node.children_iter() {
        if child.is_named() {
            docs.push(fmt_expr(f, child, &inner));
            if let Some(t) = last_token_of(child) {
                prev = Some(t);
            }
        } else {
            let tok = to_token(f, child);
            if let Some(p) = prev {
                let sep = token_sep(f, Some(&p), &tok, &inner);
                apply_sep(f, &mut docs, &p, &tok, sep);
            }
            docs.push(Doc::text(tok.text));
            prev = Some(tok);
        }
    }
    docs.push(Doc::text("]"));
    Doc::concat(docs)
}

fn fmt_call(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let mut prev: Option<Token> = None;
    for child in node.children_iter() {
        if child.is_named() {
            let d = fmt_expr(f, child, ctx);
            if let Some(p) = prev {
                if let Some(tok) = first_token_of(child) {
                    let ws = f.ws(p.byte_range.1, tok.byte_range.0);
                    // 注释后或 `(` 后换行：保留（多行调用）
                    if has_newline(ws) && (p.is_comment || p.kind == "(") {
                        docs.push(Doc::Newline);
                    } else {
                        let sep = token_sep(f, Some(&p), &tok, ctx);
                        apply_sep(f, &mut docs, &p, &tok, sep);
                    }
                }
            }
            docs.push(d);
            if let Some(t) = last_token_of(child) {
                prev = Some(t);
            }
        } else {
            let tok = to_token(f, child);
            if let Some(p) = prev {
                let ws = f.ws(p.byte_range.1, tok.byte_range.0);
                // 注释后换行：保留（多行调用）
                if has_newline(ws) && p.is_comment {
                    docs.push(Doc::Newline);
                } else {
                    let sep = token_sep(f, Some(&p), &tok, ctx);
                    apply_sep(f, &mut docs, &p, &tok, sep);
                }
            }
            docs.push(Doc::text(tok.text));
            // 左括号后：单行时无空格（`$display("...")`），超宽时参数整体
            // 换行并缩进（column_limit）。用 SoftLineNil：flat 时不输出。
            if tok.kind == "(" {
                docs.push(Doc::Indent);
                docs.push(Doc::SoftLineNil);
            } else if tok.kind == ")" {
                // 多行调用：`)` 前保留换行（在 +1 缩进层）
                if let Some(p) = prev {
                    let ws = f.ws(p.byte_range.1, tok.byte_range.0);
                    if has_newline(ws) {
                        docs.push(Doc::Newline);
                    }
                }
                // `)` 已由上方 `docs.push(Doc::text(tok.text))` 输出，此处仅缩进回退。
                docs.push(Doc::Dedent);
            }
            prev = Some(tok);
        }
    }
    Doc::concat(docs)
}

/// 赋值表达式：LHS op RHS，运算符两侧空格。
pub fn fmt_assignment(f: &Formatter<'_>, node: CstNode<'_>, ctx: &ExprCtx) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let mut prev: Option<Token> = None;
    for child in node.children_iter() {
        if child.is_named() {
            if let Some(p) = prev
                && let Some(tok) = first_token_of(child)
            {
                let sep = if f.cfg.space.around_assignment {
                    Sep::Space
                } else {
                    Sep::None
                };
                apply_sep(f, &mut docs, &p, &tok, sep);
            }
            docs.push(fmt_expr(f, child, ctx));
            if let Some(t) = last_token_of(child) {
                prev = Some(t);
            }
        } else {
            let tok = to_token(f, child);
            if let Some(p) = prev {
                let sep = if f.cfg.space.around_assignment {
                    Sep::Space
                } else {
                    Sep::None
                };
                apply_sep(f, &mut docs, &p, &tok, sep);
            }
            docs.push(Doc::text(tok.text));
            prev = Some(tok);
        }
    }
    Doc::concat(docs)
}

/// 判断节点是否需要表达式格式化（供上层 dispatch）。
pub fn dispatch_expr(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    if is_value_kind(node.kind()) {
        fmt_expr(f, node, &ExprCtx::default())
    } else {
        fmt_default(f, node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FormatterConfig;

    fn fmt_expr_src(src: &str) -> String {
        let cfg = FormatterConfig::default();
        let f = Formatter::new(&cfg, src);
        let mut parser = crate::parser::SvParser::new().unwrap();
        let tree = parser.parse(src).unwrap();
        let doc = f.fmt(tree.root_node());
        let opts = crate::document::RenderOptions::from(&cfg);
        crate::document::render(&doc, &opts)
    }

    #[test]
    fn binary_ops_get_spaces() {
        assert_eq!(
            fmt_expr_src("module t; assign c = a + b; endmodule\n"),
            "module t;\nassign c = a + b;\nendmodule\n"
        );
    }

    #[test]
    fn no_space_around_parens() {
        assert_eq!(
            fmt_expr_src("module t; assign c = (a+b)*2; endmodule\n"),
            "module t;\nassign c = (a + b) * 2;\nendmodule\n"
        );
    }

    #[test]
    fn concat_commas_have_no_space() {
        assert_eq!(
            fmt_expr_src("module t; assign y = {a, b, c}; endmodule\n"),
            "module t;\nassign y = {a, b, c};\nendmodule\n"
        );
    }

    #[test]
    fn unary_minus_no_space() {
        assert_eq!(
            fmt_expr_src("module t; assign y = -a; endmodule\n"),
            "module t;\nassign y = -a;\nendmodule\n"
        );
    }

    #[test]
    fn scope_resolution_has_no_spaces() {
        // `::` 是作用域解析符，两侧不加空格（`bsp::DEVICE_MAC`）
        assert_eq!(
            fmt_expr_src("module t; assign y = bsp::DEVICE_MAC; endmodule\n"),
            "module t;\nassign y = bsp::DEVICE_MAC;\nendmodule\n"
        );
        // 条件表达式中的 `::` 同样无空格
        assert_eq!(
            fmt_expr_src("module t; if (cnt < bsp::PERIOD_TIME_CNT) cnt = 0; endmodule\n"),
            "module t;\nif(cnt < bsp::PERIOD_TIME_CNT) cnt = 0;\nendmodule\n"
        );
    }

    #[test]
    fn macro_reference_has_no_space_between_backtick_and_name() {
        // 宏引用 `MACRO：反引号与宏名之间无空格，但运算符与反引号之间保留空格
        assert_eq!(
            fmt_expr_src(
                "module t; assign y = (flash_cmd == `FLASH_BUFFER_PROGRAM); endmodule\n"
            ),
            "module t;\nassign y = (flash_cmd == `FLASH_BUFFER_PROGRAM);\nendmodule\n"
        );
    }

    #[test]
    fn else_if_stays_on_same_line() {
        // 嵌套 conditional 的 `else if` 应连在一起（else 后不换行、不额外缩进），
        // 而不是拆成 `else` + 换行 + `if`。
        let src = "module t;\nalways @ *\n    if(A)\n        x <= 1;\n    else if(B)\n        x <= 2;\n    else\n        x <= 3;\nendmodule\n";
        let expect = "module t;\nalways @ *\n    if(A)\n        x <= 1;\n    else if(B)\n        x <= 2;\n    else\n        x <= 3;\nendmodule\n";
        assert_eq!(fmt_expr_src(src), expect);
    }
}

#[cfg(test)]
mod concat_debug {
    use super::*;
    use crate::config::FormatterConfig;
    use crate::document::{RenderOptions, render};
    use crate::parser::SvParser;

    #[test]
    fn concat_no_space() {
        let cfg = FormatterConfig::default();
        let src = "module t; assign y = {a, b, c}; endmodule";
        let f = Formatter::new(&cfg, src);
        let mut p = SvParser::new().unwrap();
        let tree = p.parse(src).unwrap();
        let doc = f.fmt(tree.root_node());
        let out = render(&doc, &RenderOptions::from(&cfg));
        assert!(out.contains("{a, b, c}"), "got: {out:?}");
    }
}
