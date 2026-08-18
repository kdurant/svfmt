//! 模块实例化与端口/参数连接对齐。

use crate::document::Doc;
use crate::formatter::expressions::{ExprCtx, fmt_default, fmt_expr};
use crate::formatter::tokens::display_width;
use crate::formatter::{Formatter, count_blank_lines};
use crate::parser::CstNode;

/// module_instantiation。
pub fn fmt_module_instantiation(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    if f.svdbg() {
        eprintln!("[MI] text={:?}", node.text().split('\n').next());
    }
    let items: Vec<CstNode<'_>> = node.children();
    let mut type_name = String::new();
    let mut params: Option<CstNode<'_>> = None;
    let mut instance: Option<CstNode<'_>> = None;
    let mut inst_name = String::new();
    let mut ports: Option<CstNode<'_>> = None;
    let mut has_empty_parens = false;

    for c in &items {
        match c.kind() {
            "simple_identifier" if type_name.is_empty() => type_name = c.text().to_string(),
            "parameter_value_assignment" => params = Some(*c),
            "module_instance" | "hierarchical_instance" => instance = Some(*c),
            _ => {}
        }
    }
    if let Some(inst) = instance {
        let mut has_parens = false;
        for c in inst.children() {
            if (c.kind() == "name_of_instance" || c.kind() == "simple_identifier")
                && inst_name.is_empty()
            {
                inst_name = c.text().to_string();
            } else if c.kind() == "(" || c.kind() == ")" {
                has_parens = true;
            } else if c.kind() == "list_of_port_connections" {
                ports = Some(c);
            }
        }
        // 空端口 `()`：不是端口列表。
        // 无论是否有参数，只要无端口连接就标记为 has_empty_parens，压缩为一行。
        if ports.is_none() && has_parens {
            has_empty_parens = true;
        }
    }

    // 无端口实例化（`type inst();`）：压缩为一行
    // tree-sitter 不区分模块与接口，故统一按"无端口"判断。
    if has_empty_parens {
        return fmt_empty_port_one_line(f, node, &type_name, params, &inst_name);
    }

    let mut docs: Vec<Doc> = Vec::new();
    // 参数与端口共享 name/value 对齐列
    let mut shared_name_max = 0usize;
    let mut shared_value_max = 0usize;
    if f.svdbg() {
        eprintln!("[inst2] type={:?}", type_name);
    }
    if let Some(p) = params {
        for c in parameter_connections_nodes(f, p) {
            let (name, value) = connection_columns(f, c);
            let w = display_width(&name, f.cfg.tab_width as usize);
            if w > shared_name_max {
                shared_name_max = w;
            }
            let vw = display_width(&value, f.cfg.tab_width as usize);
            if vw > shared_value_max {
                shared_value_max = vw;
            }
        }
    }
    if let Some(pl) = ports
        && pl.kind() == "list_of_port_connections"
    {
        for c in pl.children().into_iter().filter(|c| c.is_named()) {
            let (name, value) = connection_columns(f, c);
            let w = display_width(&name, f.cfg.tab_width as usize);
            if w > shared_name_max {
                shared_name_max = w;
            }
            let vw = display_width(&value, f.cfg.tab_width as usize);
            if vw > shared_value_max {
                shared_value_max = vw;
            }
        }
    }
    docs.push(Doc::text(type_name.clone()));
    if let Some(p) = params {
        docs.push(Doc::Space);
        docs.push(Doc::text("#"));
        if f.cfg.module.parameter_list_break_before_open_paren {
            docs.push(Doc::Newline);
        }
        docs.push(Doc::text("("));
        docs.push(Doc::Newline);
        docs.push(Doc::Indent);
        docs.push(fmt_parameter_connections(
            f,
            p,
            shared_name_max,
            shared_value_max,
        ));
        docs.push(Doc::Dedent);
        docs.push(Doc::Newline);
        docs.push(Doc::text(")"));
    }
    if params.is_none() && ports.is_none() {
        // 无参数无端口且无括号的简单实例：单行（极少触发，正常实例化均有括号）
        docs.push(Doc::Space);
        docs.push(Doc::text(inst_name.clone()));
        docs.push(Doc::text(";"));
        return Doc::concat(docs);
    }
    if params.is_none() {
        // 无参数：类型名与实例名同行
        docs.push(Doc::Space);
        docs.push(Doc::text(inst_name.clone()));
    } else {
        docs.push(Doc::Newline);
        docs.push(Doc::text(inst_name.clone()));
    }
    if let Some(pl) = ports {
        if f.cfg.module.instance_port_list_break_before_open_paren {
            docs.push(Doc::Newline);
        }
        docs.push(Doc::text("("));
        docs.push(Doc::Newline);
        docs.push(Doc::Indent);
        docs.push(fmt_port_connections(
            f,
            pl,
            shared_name_max,
            shared_value_max,
        ));
        docs.push(Doc::Dedent);
        docs.push(Doc::Newline);
        docs.push(Doc::text(")"));
    }
    docs.push(Doc::text(";"));
    Doc::concat(docs)
}

/// 无端口实例化单行输出：`type #(...) inst();` 或 `type inst();`。
fn fmt_empty_port_one_line(
    f: &Formatter<'_>,
    _node: CstNode<'_>,
    type_name: &str,
    params: Option<CstNode<'_>>,
    inst_name: &str,
) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    docs.push(Doc::text(type_name.to_string()));
    if let Some(p) = params {
        docs.push(Doc::Space);
        docs.push(Doc::text("#"));
        docs.push(Doc::text("("));
        docs.push(fmt_inline_connections(f, p));
        docs.push(Doc::text(")"));
    }
    docs.push(Doc::Space);
    docs.push(Doc::text(inst_name.to_string()));
    docs.push(Doc::text("()"));
    docs.push(Doc::text(";"));
    Doc::concat(docs)
}

/// 单行连接（接口压缩）：`.name(value)` 逗号空格分隔。
fn fmt_inline_connections(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let mut first = true;
    let prev_end: Option<usize> = None;
    for c in node.children_iter() {
        if c.is_named() {
            if !first {
                docs.push(Doc::Space);
            }
            docs.push(fmt_named_connection(f, c));
            first = false;
        } else if c.kind() == "," {
            docs.push(Doc::text(","));
        }
        let _ = prev_end;
    }
    Doc::concat(docs)
}

/// 参数连接（`#(...)` 内，多行对齐）。
fn fmt_parameter_connections(
    f: &Formatter<'_>,
    node: CstNode<'_>,
    shared_name_max: usize,
    shared_value_max: usize,
) -> Doc {
    let items: Vec<CstNode<'_>> = node.children();
    let conns: Vec<CstNode<'_>> = items
        .iter()
        .filter(|c| c.kind() == "list_of_parameter_value_assignments")
        .flat_map(|l| {
            l.children()
                .into_iter()
                .filter(|c| c.is_named())
                .collect::<Vec<_>>()
        })
        .collect();
    aligned_connections(f, &conns, shared_name_max, shared_value_max, 0)
}

/// 端口连接（`(...)` 内，多行对齐）。
fn fmt_port_connections(
    f: &Formatter<'_>,
    node: CstNode<'_>,
    shared_name_max: usize,
    shared_value_max: usize,
) -> Doc {
    let items: Vec<CstNode<'_>> = node.children();
    let conns: Vec<CstNode<'_>> = items.iter().filter(|c| c.is_named()).copied().collect();
    aligned_connections(f, &conns, shared_name_max, shared_value_max, 0)
}

/// 对齐的连接列表。
fn aligned_connections(
    f: &Formatter<'_>,
    conns: &[CstNode<'_>],
    shared_name_max: usize,
    shared_value_max: usize,
    value_pad_extra: usize,
) -> Doc {
    if conns.is_empty() {
        return Doc::Nil;
    }
    // align_instance_ports 关闭时：紧凑输出（.name(value)，不做列对齐、无括号内空格）
    let align = f.cfg.align_instance_ports;
    let inner = if align {
        f.cfg.space_inside_instance_port_parens as usize
    } else {
        0
    };
    let mut parsed: Vec<(String, ConnectionValue)> = Vec::new();
    for c in conns {
        let (name, value) = connection_parts(f, *c);
        if name.trim_end_matches('.').is_empty() {
            continue;
        }
        parsed.push((name, value));
    }
    let name_max = shared_name_max;
    let value_max = shared_value_max;
    // 单行条件：newline_per_instance_port=false 时强制单行；
    // 否则按 wrap_instance_ports 阈值（连接数不超过阈值时单行）
    let wrap = f.cfg.wrap_instance_ports as usize;
    let single_line =
        !f.cfg.module.newline_per_instance_port || (wrap >= 1 && parsed.len() <= wrap);
    if f.svdbg() {
        eprintln!(
            "[aligned2] name_max={} value_max={} extra={}",
            name_max, value_max, value_pad_extra
        );
    }
    let mut docs: Vec<Doc> = Vec::new();
    let mut rendered: Vec<(String, usize)> = Vec::new(); // (行文本, conns 索引)
    let mut pi = 0usize;
    for (ci, c) in conns.iter().enumerate() {
        if c.is_named() && c.kind().ends_with("comment") {
            // 行尾注释：追加到前一个连接行
            if let Some((line, _)) = rendered.last_mut() {
                line.push_str("  ");
                line.push_str(c.text());
            }
            continue;
        }
        if pi >= parsed.len() {
            break;
        }
        let (name, value) = &parsed[pi];
        let value_str = match value {
            ConnectionValue::Normal(d) => render_doc(f, d.clone()),
            // 拼接值不进入单行输出路径（见下方强制换行回退）。
            ConnectionValue::Concat { .. } => String::new(),
        };
        let mut line = String::new();
        if align {
            line.push_str(&pad_col(name, name_max + 1));
        } else {
            line.push_str(name);
        }
        line.push('(');
        if align {
            line.push_str(&" ".repeat(inner));
            line.push_str(&pad_col(&value_str, value_max + value_pad_extra));
            line.push_str(&" ".repeat(inner));
        } else {
            line.push_str(&value_str);
        }
        line.push(')');
        let has_comma = pi + 1 < parsed.len() || c.text().trim_end().ends_with(',');
        if has_comma {
            line.push(',');
        }
        rendered.push((line, ci));
        pi += 1;
    }
    // 超宽回退：仅基于连接本体判断（不包含行尾注释），避免注释误触发回退。
    // 拼接值（花括号端口）强制进入多行路径。
    let limit = f.cfg.column_limit as usize;
    let has_concat = parsed.iter().any(|(_, v)| !v.is_normal());
    let over_limit = limit > 0
        && parsed.iter().any(|(name, value)| match value {
            ConnectionValue::Normal(d) => {
                let value = render_doc(f, d.clone());
                let mut line = String::new();
                if align {
                    line.push_str(&pad_col(name, name_max + 1));
                } else {
                    line.push_str(name);
                }
                line.push('(');
                if align {
                    line.push_str(&" ".repeat(inner));
                    line.push_str(&pad_col(&value, value_max + value_pad_extra));
                    line.push_str(&" ".repeat(inner));
                } else {
                    line.push_str(&value);
                }
                line.push(')');
                display_width(&line, f.cfg.tab_width as usize) > limit
            }
            ConnectionValue::Concat { .. } => false,
        });
    if has_concat || over_limit {
        return wrapped_connections(f, &parsed, conns, name_max, inner);
    }
    // 输出行，行间换行/空行（单行模式用空格分隔）
    let mut prev_ci: Option<usize> = None;
    for (idx, (line, ci)) in rendered.iter().enumerate() {
        if let Some(pci) = prev_ci {
            if single_line {
                docs.push(Doc::Space);
            } else {
                let ws = f.ws(conns[pci].byte_range().end, conns[*ci].byte_range().start);
                let blanks = count_blank_lines(ws);
                if blanks > 0 {
                    docs.push(Doc::BlankLines(blanks));
                } else {
                    docs.push(Doc::Newline);
                }
            }
        }
        let _ = idx;
        docs.push(Doc::text(line.clone()));
        prev_ci = Some(*ci);
    }
    Doc::concat(docs)
}

/// 换行模式（风格 A）：保留名字对齐，值用表达式 Doc 渲染，
/// 超宽时在逗号/运算符处断行（续行缩进 +1 级）。
/// 单行连接的 ) 对齐到多行连接 Fill 末行的 ) 列。
///
/// 拼接连接（花括号端口）特殊处理：`(  {` 一行、`}  )` 一行，
/// 花括号内原文逐行保留、统一重新缩进，不受 `column_limit` 影响。
fn wrapped_connections(
    f: &Formatter<'_>,
    parsed: &[(String, ConnectionValue)],
    conns: &[CstNode<'_>],
    name_max: usize,
    inner: usize,
) -> Doc {
    let align = f.cfg.align_instance_ports;
    // ) 的目标列 = Fill 续行缩进（连接级别 +1 = 2×indent_width）+ 多行值最后一词宽度。
    // 单行连接的 ) 用 Doc::Pad 对齐到此列；多行连接的 Pad 因列已达到而自动跳过。
    // 拼接连接不参与此列计算（其 `}` 对齐到 `(` 列）。
    let continuation_col = 2 * f.cfg.indent_width as usize;
    let max_last_word: usize = parsed
        .iter()
        .filter_map(|(_, v)| match v {
            ConnectionValue::Normal(d) => Some(fill_last_word_width(d, f.cfg.tab_width as usize)),
            ConnectionValue::Concat { .. } => None,
        })
        .max()
        .unwrap_or(0);
    let pad_target = if max_last_word > 0 {
        continuation_col + max_last_word
    } else {
        0
    };
    // 拼接连接闭合 `}` 的对齐前缀（相对行首）。
    // 让 `}` 对齐到最宽普通连接 `)` 的位置，使所有连接（含拼接）的右括号垂直对齐：
    // 单行连接 `)` 在 `name_max+1 + 1 + inner + value_width + inner`，
    // 拼接 `}` 在其 `-1-inner` 处，即 `name_max+1 + value_width + inner`。
    let max_normal_value_width: usize = parsed
        .iter()
        .filter_map(|(_, v)| match v {
            ConnectionValue::Normal(d) => Some(display_width(
                &render_doc(f, d.clone()),
                f.cfg.tab_width as usize,
            )),
            ConnectionValue::Concat { .. } => None,
        })
        .max()
        .unwrap_or(0);
    let close_prefix = name_max + 1 + max_normal_value_width + inner;
    let mut docs: Vec<Doc> = Vec::new();
    let mut prev_ci: Option<usize> = None;
    let mut pi = 0usize;
    for (ci, c) in conns.iter().enumerate() {
        if c.is_named() && c.kind().ends_with("comment") {
            // 行尾注释：追加到上一个连接行
            append_conn_comment(&mut docs, c.text());
            continue;
        }
        if pi >= parsed.len() {
            break;
        }
        let (name, value) = &parsed[pi];
        if let Some(pci) = prev_ci {
            let ws = f.ws(conns[pci].byte_range().end, c.byte_range().start);
            let blanks = count_blank_lines(ws);
            if blanks > 0 {
                docs.push(Doc::BlankLines(blanks));
            } else {
                docs.push(Doc::Newline);
            }
        }
        let conn_docs = match value {
            ConnectionValue::Normal(d) => {
                let mut conn_docs: Vec<Doc> = Vec::new();
                if align {
                    conn_docs.push(Doc::text(pad_col(name, name_max + 1)));
                } else {
                    conn_docs.push(Doc::text(name.clone()));
                }
                conn_docs.push(Doc::text("("));
                if align {
                    conn_docs.push(Doc::text(" ".repeat(inner)));
                }
                // 值：Indent 包裹使续行缩进 +1 级；值 Doc 自带 SoftLine 断行点
                conn_docs.push(Doc::Indent);
                conn_docs.push(d.clone());
                conn_docs.push(Doc::Dedent);
                if align && pad_target > 0 {
                    conn_docs.push(Doc::Pad(pad_target));
                }
                if align {
                    conn_docs.push(Doc::text(" ".repeat(inner)));
                }
                conn_docs.push(Doc::text(")"));
                conn_docs
            }
            ConnectionValue::Concat { inner_lines } => {
                let mut conn_docs: Vec<Doc> = Vec::new();
                if align {
                    conn_docs.push(Doc::text(pad_col(name, name_max + 1)));
                } else {
                    conn_docs.push(Doc::text(name.clone()));
                }
                conn_docs.push(Doc::text("("));
                if align {
                    conn_docs.push(Doc::text(" ".repeat(inner)));
                }
                conn_docs.push(Doc::text("{"));
                // 花括号内部：原文逐行保留，统一缩进 +1 级，column_limit 不介入。
                conn_docs.push(Doc::Newline);
                conn_docs.push(Doc::Indent);
                for (i, line) in inner_lines.iter().enumerate() {
                    if i > 0 {
                        conn_docs.push(Doc::Newline);
                    }
                    conn_docs.push(Doc::text(line.clone()));
                }
                conn_docs.push(Doc::Dedent);
                // 闭合 `}` 另起一行，并对齐到单行连接右括号 `)` 的位置，
                // 使所有连接（含拼接）的右括号垂直对齐。
                conn_docs.push(Doc::Newline);
                if align {
                    conn_docs.push(Doc::text(" ".repeat(close_prefix)));
                    conn_docs.push(Doc::text("}"));
                    conn_docs.push(Doc::text(" ".repeat(inner)));
                } else {
                    conn_docs.push(Doc::text("}"));
                }
                conn_docs.push(Doc::text(")"));
                conn_docs
            }
        };
        let has_comma = pi + 1 < parsed.len() || c.text().trim_end().ends_with(',');
        let mut final_docs = conn_docs;
        if has_comma {
            final_docs.push(Doc::text(","));
        }
        docs.push(Doc::concat(final_docs));
        prev_ci = Some(ci);
        pi += 1;
    }
    Doc::concat(docs)
}

/// 返回 Fill doc 最后一个 SoftLine 之后所有 Text 的总宽度。
/// 用于计算续行末尾的列位置，从而对齐单行连接的右括号。
fn fill_last_word_width(doc: &Doc, tab_width: usize) -> usize {
    let Doc::Fill(children) = doc else { return 0 };
    let last_sl = children
        .iter()
        .rposition(|c| matches!(c, Doc::SoftLine | Doc::SoftLineNil));
    let Some(idx) = last_sl else { return 0 };
    children[idx + 1..]
        .iter()
        .map(|c| {
            if let Doc::Text(s) = c {
                display_width(s, tab_width)
            } else {
                0
            }
        })
        .sum()
}

/// 把行尾注释追加到上一个连接 Doc 的末尾。
fn append_conn_comment(docs: &mut Vec<Doc>, comment: &str) {
    if let Some(last) = docs.last_mut() {
        if let Doc::Group(children) = last {
            if let Some(Doc::Text(s)) = children.last_mut() {
                s.push_str("  ");
                s.push_str(comment);
            }
        }
    }
}

fn pad_col(text: &str, width: usize) -> String {
    let w = display_width(text, 4);
    let mut out = String::from(text);
    if w < width {
        for _ in 0..(width - w) {
            out.push(' ');
        }
    }
    out
}

/// 端口/参数连接的值。
enum ConnectionValue {
    /// 普通值：用 Doc 渲染（单行或按 column_limit 断行）。
    Normal(Doc),
    /// 拼接 `{...}`：端口值含花括号时，完全保留原文、多行输出。
    ///
    /// `inner_lines` 为花括号内原文按行拆分（已去掉每行行首缩进），
    /// 排版时统一重新缩进，且不受 `column_limit` 影响。
    Concat { inner_lines: Vec<String> },
}

impl ConnectionValue {
    /// 是否为普通单行值。
    fn is_normal(&self) -> bool {
        matches!(self, ConnectionValue::Normal(_))
    }
}

/// 提取连接的 name（`.port`）与 value。
fn connection_parts(f: &Formatter<'_>, node: CstNode<'_>) -> (String, ConnectionValue) {
    let items: Vec<CstNode<'_>> = node.children();
    let mut name = String::new();
    let mut value = ConnectionValue::Normal(Doc::Nil);
    if f.svdbg() {
        eprintln!(
            "[conncols] node_kind={} items={:?}",
            node.kind(),
            items.iter().map(|c| c.kind()).collect::<Vec<_>>()
        );
    }
    for c in &items {
        match c.kind() {
            "named_port_connection" | "named_parameter_assignment" => {
                // 递归解析
                let sub: Vec<CstNode<'_>> = c.children();
                for s in &sub {
                    if s.kind() == "." {
                        continue;
                    }
                    if s.kind() == "simple_identifier" && name.is_empty() {
                        name = s.text().to_string();
                    } else if s.kind() == "param_expression"
                        || s.kind() == "expression"
                        || s.kind() == "mintypmax_expression"
                    {
                        value = fmt_connection_value(f, *s);
                    }
                }
            }
            "ordered_port_connection" | "ordered_parameter_assignment" => {
                let sub: Vec<CstNode<'_>> = c.children();
                for s in &sub {
                    if s.is_named() && s.kind() != "constant_expression" && s.kind() != "expression"
                    {
                        value = ConnectionValue::Normal(Doc::text(s.text().to_string()));
                    }
                }
            }
            "." => {}
            "simple_identifier" if name.is_empty() => name = c.text().to_string(),
            "param_expression" | "expression" | "mintypmax_expression" => {
                value = fmt_connection_value(f, *c);
            }
            "(" | ")" => {}
            _ => {}
        }
    }
    (format!(".{name}"), value)
}

/// 格式化连接值：若值（子）节点是花括号拼接，则提取保留原文的多行内容；
/// 否则走常规表达式格式化。
fn fmt_connection_value(f: &Formatter<'_>, node: CstNode<'_>) -> ConnectionValue {
    if let Some(inner) = concat_inner_lines(node) {
        return ConnectionValue::Concat { inner_lines: inner };
    }
    ConnectionValue::Normal(fmt_expr(f, node, &ExprCtx::default()))
}

/// 在值节点子树中查找 `concatenation`/`streaming_concatenation`。
///
/// 端口值形如 `.port ( {a, b, c} )`，CST 结构为
/// `expression -> primary -> concatenation`，故需要递归查找。
fn find_concat(node: CstNode<'_>) -> Option<CstNode<'_>> {
    if node.kind() == "concatenation" || node.kind() == "streaming_concatenation" {
        return Some(node);
    }
    for c in node.children() {
        if let Some(found) = find_concat(c) {
            return Some(found);
        }
    }
    None
}

/// 若值节点含花括号拼接，返回花括号内原文按行拆分（每行去除行首空白）。
/// 内部行内容（含行内间距）原样保留，仅去掉每行的前导缩进以便统一重排。
fn concat_inner_lines(node: CstNode<'_>) -> Option<Vec<String>> {
    let concat = find_concat(node)?;
    let text = concat.text();
    // 去掉最外层的一对 `{ ... }`
    let t = text.trim();
    if !t.starts_with('{') || !t.ends_with('}') {
        return None;
    }
    let inner = &t[1..t.len() - 1];
    let mut lines: Vec<String> = Vec::new();
    for raw in inner.split('\n') {
        lines.push(raw.trim_start().to_string());
    }
    Some(lines)
}

/// 提取连接的 name（`.port`）与 value（括号内表达式，预渲染为单行字符串）。
fn connection_columns(f: &Formatter<'_>, node: CstNode<'_>) -> (String, String) {
    let (name, value) = connection_parts(f, node);
    let vs = match &value {
        ConnectionValue::Normal(d) => render_doc(f, d.clone()),
        // 拼接值用于单行宽度测量时贡献为 0（拼接总是多行输出）。
        ConnectionValue::Concat { .. } => String::new(),
    };
    (name, vs)
}

/// named 连接格式化（供内联使用）。
fn fmt_named_connection(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let _ = f;
    if node.kind() == "named_port_connection" || node.kind() == "named_parameter_assignment" {
        let (name, value) = connection_columns(f, node);
        let mut docs: Vec<Doc> = vec![Doc::text(name)];
        if !value.is_empty() {
            docs.push(Doc::text("("));
            docs.push(Doc::text(value));
            docs.push(Doc::text(")"));
        }
        Doc::concat(docs)
    } else {
        fmt_default(f, node)
    }
}

/// 供表达式使用的辅助。
pub fn _unused(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    fmt_expr(f, node, &ExprCtx::default())
}

/// 提取参数连接的节点列表。
fn parameter_connections_nodes<'a>(_f: &Formatter<'a>, node: CstNode<'a>) -> Vec<CstNode<'a>> {
    let items: Vec<CstNode<'a>> = node.children();
    items
        .iter()
        .filter(|c| c.kind() == "list_of_parameter_value_assignments")
        .flat_map(|l| {
            l.children()
                .into_iter()
                .filter(|c| c.is_named())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn render_doc(f: &Formatter<'_>, doc: crate::document::Doc) -> String {
    crate::document::render_inline(&doc, f.cfg)
}
