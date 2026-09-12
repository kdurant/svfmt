//! 模块实例化与端口/参数连接对齐。

use crate::document::Doc;
use crate::formatter::expressions::{ExprCtx, fmt_expr};
use crate::formatter::tokens::display_width;
use crate::formatter::{Formatter, count_blank_lines};
use crate::parser::CstNode;

/// 一个实例：`name [ ( ports ) ]`。
struct Instance<'t> {
    name: String,
    ports: Option<CstNode<'t>>,
    has_parens: bool,
}

/// module_instantiation。
///
/// 同一个类型可以声明多个实例（`foo a(), b();`），因此必须收集**全部**
/// `module_instance`/`hierarchical_instance`；此前只保留最后一个，会静默
/// 丢掉前面的实例（等于少例化一个模块）。
pub fn fmt_module_instantiation(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    if f.svdbg() {
        eprintln!("[MI] text={:?}", node.text().split('\n').next());
    }
    let items: Vec<CstNode<'_>> = node.children();
    let mut type_name = String::new();
    let mut params: Option<CstNode<'_>> = None;
    let mut instances: Vec<Instance<'_>> = Vec::new();
    for c in &items {
        match c.kind() {
            "simple_identifier" if type_name.is_empty() => type_name = c.text().to_string(),
            "parameter_value_assignment" => params = Some(*c),
            "module_instance" | "hierarchical_instance" => {
                let mut name = String::new();
                let mut ports = None;
                let mut has_parens = false;
                for k in c.children() {
                    match k.kind() {
                        "name_of_instance" | "simple_identifier" if name.is_empty() => {
                            name = k.text().to_string();
                        }
                        "list_of_port_connections" => ports = Some(k),
                        "(" | ")" => has_parens = true,
                        _ => {}
                    }
                }
                instances.push(Instance {
                    name,
                    ports,
                    has_parens,
                });
            }
            _ => {}
        }
    }
    if instances.is_empty() {
        return f.raw(node);
    }
    // 全部实例都是空端口（`()`）：压缩为一行（保持既有行为）
    // tree-sitter 不区分模块与接口，故统一按"无端口"判断。
    if instances.iter().all(|i| i.ports.is_none() && i.has_parens) {
        return fmt_empty_port_one_line(f, &type_name, params, &instances);
    }
    let tw = f.cfg.tab_width as usize;
    // 参数与所有实例的端口共享 name/value 对齐列
    let mut shared_name_max = 0usize;
    let mut shared_value_max = 0usize;
    if f.svdbg() {
        eprintln!("[inst2] type={:?}", type_name);
    }
    if let Some(p) = params {
        for c in parameter_connections_nodes(f, p) {
            let (name, value) = connection_columns(f, c);
            let w = display_width(&name, tw);
            if w > shared_name_max {
                shared_name_max = w;
            }
            let vw = display_width(&value, tw);
            if vw > shared_value_max {
                shared_value_max = vw;
            }
        }
    }
    for inst in &instances {
        let Some(pl) = inst.ports else { continue };
        for c in pl.children().into_iter().filter(|c| c.is_named()) {
            let (name, value) = connection_columns(f, c);
            let w = display_width(&name, tw);
            if w > shared_name_max {
                shared_name_max = w;
            }
            let vw = display_width(&value, tw);
            if vw > shared_value_max {
                shared_value_max = vw;
            }
        }
    }
    let mut docs: Vec<Doc> = Vec::new();
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
    for (i, inst) in instances.iter().enumerate() {
        if i == 0 {
            // 无参数：类型名与实例名同行；有参数：实例名另起一行
            if params.is_some() {
                docs.push(Doc::Newline);
            } else {
                docs.push(Doc::Space);
            }
        } else {
            // 多实例：前一个实例的 `)` 之后加逗号再换行
            docs.push(Doc::text(","));
            docs.push(Doc::Newline);
        }
        docs.push(Doc::text(inst.name.clone()));
        if let Some(pl) = inst.ports {
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
        } else if inst.has_parens {
            docs.push(Doc::text("()"));
        }
    }
    docs.push(Doc::text(";"));
    Doc::concat(docs)
}

/// 无端口实例化单行输出：`type #(...) inst();` / `type a(), b();`。
fn fmt_empty_port_one_line(
    f: &Formatter<'_>,
    type_name: &str,
    params: Option<CstNode<'_>>,
    instances: &[Instance<'_>],
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
    for (i, inst) in instances.iter().enumerate() {
        docs.push(Doc::Space);
        docs.push(Doc::text(inst.name.clone()));
        docs.push(Doc::text("()"));
        if i + 1 < instances.len() {
            docs.push(Doc::text(","));
        }
    }
    docs.push(Doc::text(";"));
    Doc::concat(docs)
}

/// 单行连接（接口压缩）：`.name(value)` 逗号空格分隔。
fn fmt_inline_connections(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let mut first = true;
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
    let tw = f.cfg.tab_width as usize;
    let inner = if align {
        f.cfg.space_inside_instance_port_parens as usize
    } else {
        0
    };
    let mut parsed: Vec<Conn> = Vec::new();
    for c in conns {
        let conn = connection_parts(f, *c);
        // `.*` 与位置连接必须保留；仅跳过"名字解析失败的命名连接"（防御性）。
        if let ConnForm::Named(n) = &conn.form
            && n.trim_end_matches('.').is_empty()
        {
            continue;
        }
        parsed.push(conn);
    }
    let name_max = shared_name_max;
    let value_max = shared_value_max;
    // 单行条件：newline_per_instance_port=false 时强制单行；
    // 否则按 wrap_instance_ports 阈值（连接数不超过阈值时单行）
    let wrap = f.cfg.wrap_instance_ports as usize;
    let has_macro = conns.iter().any(|c| c.kind() == "text_macro_usage");
    let single_line = !f.cfg.module.newline_per_instance_port
        || (wrap >= 1 && parsed.len() <= wrap && !has_macro);
    if f.svdbg() {
        eprintln!(
            "[aligned2] name_max={} value_max={} extra={}",
            name_max, value_max, value_pad_extra
        );
    }
    let mut docs: Vec<Doc> = Vec::new();
    let mut rendered: Vec<(String, usize, bool, usize, usize)> = Vec::new(); // (行文本, conns 索引, 预处理指令行, 起始, 结束)
    let mut pi = 0usize;
    for (ci, c) in conns.iter().enumerate() {
        if c.is_named() && c.kind().ends_with("comment") {
            // 行尾注释：追加到前一个连接行
            if let Some((line, _, _, _, _)) = rendered.last_mut() {
                line.push_str("  ");
                line.push_str(c.text());
            }
            continue;
        }
        let r = c.byte_range();
        let dir: Option<CstNode<'_>> = if c.kind() == "text_macro_usage" {
            Some(*c)
        } else {
            c.children()
                .into_iter()
                .find(|k| k.kind() == "conditional_compilation_directive")
        };
        if let Some(d) = dir {
            // 预处理指令（`ifdef/else/endif）：顶格独立行（不参与对齐，不消耗 parsed）
            let dr = d.byte_range();
            rendered.push((d.text().trim_end().to_string(), ci, true, dr.start, dr.end));
        }
        if pi >= parsed.len() {
            break;
        }
        let conn = &parsed[pi];
        let mut line = conn_line_text(
            f,
            conn,
            align,
            name_max,
            value_max,
            value_pad_extra,
            inner,
        );
        let has_comma = pi + 1 < parsed.len() || c.text().trim_end().ends_with(',');
        if has_comma {
            line.push(',');
        }
        // 连接行起点：指令存在时取指令后下一个子节点（连接实际开始），否则节点起点
        let dir_start = if let Some(d) = dir {
            c.children()
                .into_iter()
                .find(|k| k.byte_range().start > d.byte_range().end)
                .map(|k| k.byte_range().start)
                .unwrap_or(d.byte_range().end)
        } else {
            r.start
        };
        rendered.push((line, ci, false, dir_start, r.end));
        pi += 1;
    }
    // 超宽回退：仅基于连接本体判断（不包含行尾注释），避免注释误触发回退。
    // 拼接值（花括号端口）强制进入多行路径。
    let limit = f.cfg.column_limit as usize;
    let has_concat = parsed
        .iter()
        .any(|c| matches!(c.value, ConnectionValue::Concat { .. }));
    let over_limit = limit > 0
        && parsed.iter().any(|c| match &c.form {
            ConnForm::Named(_) => match &c.value {
                ConnectionValue::Normal(_) => {
                    let line = conn_line_text(
                        f,
                        c,
                        align,
                        name_max,
                        value_max,
                        value_pad_extra,
                        inner,
                    );
                    display_width(&line, tw) > limit
                }
                ConnectionValue::Concat { .. } => false,
            },
            // 位置连接：按值本身宽度判断
            ConnForm::Ordered => match &c.value {
                ConnectionValue::Normal(_) => display_width(&c.value_text(f), tw) > limit,
                ConnectionValue::Concat { .. } => false,
            },
            ConnForm::Wildcard => false,
        });
    if has_concat || over_limit {
        return wrapped_connections(f, &parsed, conns, name_max, value_max, inner);
    }
    // 输出行，行间换行/空行（单行模式用空格分隔）
    let mut prev_range: Option<(usize, usize)> = None;
    for (line, ci, is_macro, start, end) in &rendered {
        if let Some((_, pe)) = prev_range {
            if single_line {
                docs.push(Doc::Space);
            } else {
                let ws = f.ws(pe, *start);
                let blanks = count_blank_lines(ws);
                if blanks > 0 {
                    docs.push(Doc::BlankLines(blanks));
                } else {
                    docs.push(Doc::Newline);
                }
            }
        }
        if *is_macro {
            // 预处理指令顶格：Dedent → 行 → Indent
            docs.push(Doc::Dedent);
            docs.push(Doc::text(line.clone()));
            docs.push(Doc::Indent);
        } else {
            docs.push(Doc::text(line.clone()));
        }
        prev_range = Some((*ci, *end));
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
    parsed: &[Conn],
    conns: &[CstNode<'_>],
    name_max: usize,
    value_max: usize,
    inner: usize,
) -> Doc {
    let align = f.cfg.align_instance_ports;
    let tw = f.cfg.tab_width as usize;
    // ) 的目标列 = Fill 续行缩进（连接级别 +1 = 2×indent_width）+ 多行值最后一词宽度。
    // 单行连接的 ) 用 Doc::Pad 对齐到此列；多行连接的 Pad 因列已达到而自动跳过。
    // 拼接连接不参与此列计算（其 `}` 对齐到 `(` 列）。
    let continuation_col = 2 * f.cfg.indent_width as usize;
    let max_last_word: usize = parsed
        .iter()
        .filter_map(|c| match &c.value {
            ConnectionValue::Normal(d) => Some(fill_last_word_width(d, tw)),
            ConnectionValue::Concat { .. } => None,
        })
        .max()
        .unwrap_or(0);
    let pad_target = if max_last_word > 0 {
        continuation_col + max_last_word
    } else {
        0
    };
    // 单行普通值右括号的对齐列：`name_max+1 + '(' + inner + value_max + inner`，
    // 即 `(` 所在列（name_max+1）右侧 name 后 1 列 '(' 加内边距与最宽值宽度。
    // 当没有多行 Fill 值（pad_target == 0）时用它对齐单行普通值的 `)`；
    // 存在多行值时取二者较大者，保证两类连接的右括号不右移丢失对齐。
    let single_line_pad = name_max + 2 + inner + value_max;
    let effective_pad = pad_target.max(single_line_pad);
    // 拼接连接闭合 `}` 的对齐前缀（相对行首）。
    // 让 `}` 对齐到最宽普通连接 `)` 的位置，使所有连接（含拼接）的右括号垂直对齐。
    // 单行连接 `)` 相对内容列 = `name_max+1 + 1 + inner + value_max + inner`，
    // 拼接 `}` 在其 `-1-inner` 处，即 `name_max+1 + value_max + inner`。
    // 使用共享 value_max（含参数段）而非端口段局部最大值，保证与普通值实际
    // 对齐列（value_max 列）一致。
    let close_prefix = name_max + 1 + value_max + inner;
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
        let conn = &parsed[pi];
        if let Some(pci) = prev_ci {
            let ws = f.ws(conns[pci].byte_range().end, c.byte_range().start);
            let blanks = count_blank_lines(ws);
            if blanks > 0 {
                docs.push(Doc::BlankLines(blanks));
            } else {
                docs.push(Doc::Newline);
            }
        }
        let conn_docs = match &conn.form {
            // 通配端口连接 `.*`：单独一行，无括号与值。
            ConnForm::Wildcard => vec![Doc::text(".*")],
            // 位置连接：只有值（续行缩进 +1 级）
            ConnForm::Ordered => {
                let mut d: Vec<Doc> = Vec::new();
                match &conn.value {
                    ConnectionValue::Normal(doc) => {
                        d.push(Doc::Indent);
                        d.push(doc.clone());
                        d.push(Doc::Dedent);
                    }
                    ConnectionValue::Concat { inner_lines } => {
                        d.push(Doc::text("{"));
                        d.push(Doc::Newline);
                        d.push(Doc::Indent);
                        for (i, line) in inner_lines.iter().enumerate() {
                            if i > 0 {
                                d.push(Doc::Newline);
                            }
                            d.push(Doc::text(line.clone()));
                        }
                        d.push(Doc::Dedent);
                        d.push(Doc::Newline);
                        d.push(Doc::text("}"));
                    }
                }
                d
            }
            ConnForm::Named(name) => {
                if !conn.parens {
                    // 隐式命名连接 `.name`：源无括号与值，不得补 `()`。
                    vec![Doc::text(name.clone())]
                } else {
                    match &conn.value {
                ConnectionValue::Normal(doc) => {
                    // 单行普通值：值渲染为单行、且宽度不超过 value_max（无需断行）时，
                    // 用字符串相对对齐到 value_max（与 aligned_connections 单行路径一致）。
                    // 相对对齐不受外层缩进影响，避免 Doc::Pad 用绝对列在模块缩进下失效。
                    let value_str = render_doc(f, doc.clone());
                    let single_line_ok = align
                        && !value_str.contains('\n')
                        && display_width(&value_str, tw) <= value_max;
                    if single_line_ok {
                        let mut line = pad_col(name, name_max + 1, tw);
                        line.push('(');
                        line.push_str(&" ".repeat(inner));
                        line.push_str(&pad_col(&value_str, value_max, tw));
                        line.push_str(&" ".repeat(inner));
                        line.push(')');
                        vec![Doc::text(line)]
                    } else {
                        let mut conn_docs: Vec<Doc> = Vec::new();
                        if align {
                            conn_docs.push(Doc::text(pad_col(name, name_max + 1, tw)));
                        } else {
                            conn_docs.push(Doc::text(name.clone()));
                        }
                        conn_docs.push(Doc::text("("));
                        if align {
                            conn_docs.push(Doc::text(" ".repeat(inner)));
                        }
                        // 值：Indent 包裹使续行缩进 +1 级；值 Doc 自带 SoftLine 断行点
                        conn_docs.push(Doc::Indent);
                        conn_docs.push(doc.clone());
                        conn_docs.push(Doc::Dedent);
                        if align && effective_pad > 0 {
                            conn_docs.push(Doc::Pad(effective_pad));
                        }
                        if align {
                            conn_docs.push(Doc::text(" ".repeat(inner)));
                        }
                        conn_docs.push(Doc::text(")"));
                        conn_docs
                    }
                }
                ConnectionValue::Concat { inner_lines } => {
                    let mut conn_docs: Vec<Doc> = Vec::new();
                    if align {
                        conn_docs.push(Doc::text(pad_col(name, name_max + 1, tw)));
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
                    }
                }
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
fn append_conn_comment(docs: &mut [Doc], comment: &str) {
    if let Some(last) = docs.last_mut()
        && let Doc::Group(children) = last
        && let Some(Doc::Text(s)) = children.last_mut()
    {
        s.push_str("  ");
        s.push_str(comment);
    }
}

fn pad_col(text: &str, width: usize, tab_width: usize) -> String {
    let w = display_width(text, tab_width);
    let mut out = String::from(text);
    if w < width {
        for _ in 0..(width - w) {
            out.push(' ');
        }
    }
    out
}

/// 单行连接文本。
///
/// 隐式命名连接（`.a`，源中无括号）只输出 `name`：无括号即无值列，若仍按列
/// 补空格会留下尾随空白。其余命名连接按 `align` 决定是否列对齐。
fn conn_line_text(
    f: &Formatter<'_>,
    conn: &Conn,
    align: bool,
    name_max: usize,
    value_max: usize,
    value_pad_extra: usize,
    inner: usize,
) -> String {
    let tw = f.cfg.tab_width as usize;
    let mut line = String::new();
    match &conn.form {
        ConnForm::Wildcard => line.push_str(".*"),
        ConnForm::Ordered => line.push_str(&conn.value_text(f)),
        ConnForm::Named(name) => {
            if !conn.parens {
                line.push_str(name);
                return line;
            }
            let value_str = match &conn.value {
                ConnectionValue::Normal(d) => render_doc(f, d.clone()),
                // 拼接值不进入单行输出路径（调用方强制走多行路径）。
                ConnectionValue::Concat { .. } => String::new(),
            };
            if align {
                line.push_str(&pad_col(name, name_max + 1, tw));
            } else {
                line.push_str(name);
            }
            line.push('(');
            if align {
                line.push_str(&" ".repeat(inner));
                line.push_str(&pad_col(&value_str, value_max + value_pad_extra, tw));
                line.push_str(&" ".repeat(inner));
            } else {
                line.push_str(&value_str);
            }
            line.push(')');
        }
    }
    line
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

/// 连接的形式。
enum ConnForm {
    /// 命名连接 `.name(value)`
    Named(String),
    /// 位置（ordered）连接：只有值，没有 `.name(...)` 包装。
    ///
    /// 如 `foo u(a, b)`、`foo #(8) u(...)`——必须保留，否则连接/参数被静默丢弃。
    Ordered,
    /// 通配连接 `.*`：无端口名、无值、无括号。
    ///
    /// 若被当作"无名连接"过滤掉会静默丢失连接语义（见 examples/ram_sdp.sv）。
    Wildcard,
}

/// 一条端口/参数连接。
struct Conn {
    form: ConnForm,
    value: ConnectionValue,
    /// 源文本中是否带括号。隐式命名连接 `.a`（等价 `.a(a)`）**没有**括号与值，
    /// 必须保持原样——无条件补 `()` 会新增 token 并改变连接语义。
    parens: bool,
}

impl Conn {
    /// 连接名（用于列宽统计；ordered 连接没有名字）。
    fn name(&self) -> &str {
        match &self.form {
            ConnForm::Named(n) => n,
            ConnForm::Wildcard => ".*",
            ConnForm::Ordered => "",
        }
    }

    /// 值的显示文本（单行；拼接值按行拼接）。
    fn value_text(&self, f: &Formatter<'_>) -> String {
        match &self.value {
            ConnectionValue::Normal(d) => render_doc(f, d.clone()),
            ConnectionValue::Concat { inner_lines } => {
                let mut s = String::from("{");
                s.push_str(&inner_lines.join("\n"));
                s.push('}');
                s
            }
        }
    }
}

/// 提取连接的形式与值。
fn connection_parts(f: &Formatter<'_>, node: CstNode<'_>) -> Conn {
    let items: Vec<CstNode<'_>> = node.children();
    // 通配端口连接 `.*`：CST 为 named_port_connection 下单个 `.*` token，
    // 既无端口名也无值，必须特判保留（否则连接语义会被静默丢弃）。
    if items.iter().any(|c| c.kind() == ".*") || node.text().trim() == ".*" {
        return Conn {
            form: ConnForm::Wildcard,
            value: ConnectionValue::Normal(Doc::Nil),
            parens: false,
        };
    }
    // 位置（ordered）连接：`foo u(a, b)` / `foo #(8) u(...)`
    if matches!(
        node.kind(),
        "ordered_port_connection" | "ordered_parameter_assignment"
    ) {
        return Conn {
            form: ConnForm::Ordered,
            value: ordered_connection_value(f, node),
            parens: false,
        };
    }
    let mut name = String::new();
    let mut value = ConnectionValue::Normal(Doc::Nil);
    let mut parens = false;
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
                    } else if s.kind() == "(" {
                        parens = true;
                    }
                }
            }
            "." => {}
            "simple_identifier" if name.is_empty() => name = c.text().to_string(),
            "param_expression" | "expression" | "mintypmax_expression" => {
                value = fmt_connection_value(f, *c);
            }
            "(" => parens = true,
            ")" => {}
            _ => {}
        }
    }
    // 空名（注释、宏、异常结构）：保持缺失标记，调用方据此跳过
    // （宏/预处理指令由调用方的指令分支单独输出）。
    Conn {
        form: ConnForm::Named(format!(".{name}")),
        value,
        parens,
    }
}

/// 位置连接的值：第一个 named 子节点（`expression` / `param_expression`）。
fn ordered_connection_value(f: &Formatter<'_>, node: CstNode<'_>) -> ConnectionValue {
    for c in node.children_iter() {
        if c.is_named() {
            return fmt_connection_value(f, c);
        }
    }
    ConnectionValue::Normal(Doc::text(node.text().trim().to_string()))
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

/// 提取连接的 name 与 value（预渲染为单行字符串），用于列宽统计。
fn connection_columns(f: &Formatter<'_>, node: CstNode<'_>) -> (String, String) {
    let conn = connection_parts(f, node);
    let vs = match &conn.value {
        ConnectionValue::Normal(d) => render_doc(f, d.clone()),
        // 拼接值用于单行宽度测量时贡献为 0（拼接总是多行输出）。
        ConnectionValue::Concat { .. } => String::new(),
    };
    (conn.name().to_string(), vs)
}

/// 单个连接的单行格式化（供内联使用）。
fn fmt_named_connection(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    // 非单条连接节点（如整个 `list_of_parameter_value_assignments`）：原文输出，
    // 否则会只保留其中最后一条连接（丢弃其余）。
    if !matches!(
        node.kind(),
        "named_port_connection"
            | "named_parameter_assignment"
            | "ordered_port_connection"
            | "ordered_parameter_assignment"
    ) {
        return f.fmt_default(node);
    }
    let conn = connection_parts(f, node);
    match &conn.form {
        // 通配连接：`.*`
        ConnForm::Wildcard => Doc::text(".*"),
        // 位置连接：只有值，没有 `.name(...)` 包装
        ConnForm::Ordered => Doc::text(conn.value_text(f)),
        ConnForm::Named(name) => {
            // 隐式命名连接 `.name`：源无括号，只输出名字。
            if !conn.parens {
                return Doc::text(name.clone());
            }
            let value = conn.value_text(f);
            let mut docs: Vec<Doc> = vec![Doc::text(name.clone())];
            docs.push(Doc::text("("));
            docs.push(Doc::text(value));
            docs.push(Doc::text(")"));
            Doc::concat(docs)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FormatterConfig;
    use crate::document::{RenderOptions, render};
    use crate::parser::SvParser;

    fn fmt_module_src(src: &str) -> String {
        let cfg = FormatterConfig::default();
        let f = Formatter::new(&cfg, src);
        let mut parser = SvParser::new().unwrap();
        let tree = parser.parse(src).unwrap();
        let doc = f.fmt(tree.root_node());
        let opts = RenderOptions::from(&cfg);
        render(&doc, &opts)
    }

    /// 普通多行端口连接：name 与 value 分别列对齐，右括号垂直对齐。
    #[test]
    fn aligned_port_connections() {
        let src = "module t;\nfoo u(\n.a(clk),\n.bbb(rst),\n.ccc(dout)\n);\nendmodule\n";
        let out = fmt_module_src(src);
        // name 列对齐（最宽 `.bbb`=4 → `(` 在第 5 列）；
        // value 列对齐（最宽 `dout`=4 → inner+value_max+inner 固定宽度）。
        assert!(out.contains("    .a   (  clk   ),\n"), "got:\n{out}");
        assert!(out.contains("    .bbb (  rst   ),\n"), "got:\n{out}");
        assert!(out.contains("    .ccc (  dout  )\n"), "got:\n{out}");
    }

    /// 含花括号拼接端口时，普通单行值的右括号仍按共享 value_max 对齐。
    /// 回归：此前 wrapped_connections 对普通单行值不按 value_max pad，右括号错位。
    #[test]
    fn concat_port_keeps_plain_values_aligned() {
        let src = "module t;\nfoo u(\n.a(clk),\n.b({x, y}),\n.ccc(mem_data)\n);\nendmodule\n";
        let out = fmt_module_src(src);
        // 普通值最宽 `mem_data`=8 → value_max=8，`.a` 的 `clk`(3) pad 到 8 宽。
        // `.a   (  clk       )`：clk 后补 5 空格 + inner 2 = 7 空格
        assert!(out.contains("    .a   (  clk       ),\n"), "got:\n{out}");
        assert!(out.contains("    .ccc (  mem_data  )\n"), "got:\n{out}");
        // concat 的闭合 `}` 单独一行（花括号值多行排版）
        assert!(out.contains("}  ),\n"), "got:\n{out}");
    }

    /// 含参数段时，concat 的 `}` 用共享 value_max（含参数段更宽值）对齐，
    /// 而非端口段局部最大值。回归：此前用端口段局部 max 导致 `}` 比普通值右括号靠左。
    #[test]
    fn concat_close_brace_uses_shared_value_max_with_params() {
        let src = "module t;\nfoo #(\n.PARAM_LONG(very_long_value_here)\n) u(\n.a(clk),\n.b({x, y})\n);\nendmodule\n";
        let out = fmt_module_src(src);
        // 共享 value_max 由参数段 `very_long_value_here`(20) 决定，
        // 端口段普通值 `.a` 的 `clk` pad 到 20 宽（补 17 + inner 2 = 19 空格）。
        assert!(
            out.contains("    .a          (  clk                   ),\n"),
            "got:\n{out}"
        );
        // concat `}` 对齐到普通值右括号列（`.b` 为最后一个连接，无逗号）
        assert!(out.contains("}  )\n"), "got:\n{out}");
    }
}
