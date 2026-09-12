//! 模块实例化与端口/参数连接对齐。

use crate::document::Doc;
use crate::formatter::expressions::{ExprCtx, fmt_expr};
use crate::formatter::tokens::{display_width, leaf_tokens};
use crate::formatter::{Formatter, count_blank_lines};
use crate::parser::CstNode;

/// 一个实例：`name [ ( ports ) ]`。
struct Instance<'t> {
    name: String,
    ports: Option<CstNode<'t>>,
    has_parens: bool,
    /// 实例级"额外"子节点（主要是注释）：既非实例名、也非端口列表或括号。
    ///
    /// 必须显式输出，否则注释被静默丢弃——tree-sitter 把 `u(/* c */)` 的注释
    /// 挂在 `hierarchical_instance` 下（空端口列表没有 `list_of_port_connections`
    /// 节点承接），把 `u(/* c */ .a(1))` 的注释挂在端口列表**之前**。
    extras: Vec<CstNode<'t>>,
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
    // `parameter_value_assignment` 下、赋值列表之外的子节点（注释）
    let mut param_extras: Vec<CstNode<'_>> = Vec::new();
    let mut instances: Vec<Instance<'_>> = Vec::new();
    for c in &items {
        match c.kind() {
            "simple_identifier" if type_name.is_empty() => type_name = c.text().to_string(),
            "parameter_value_assignment" => {
                params = Some(*c);
                for k in c.children() {
                    if k.is_named() && k.kind() != "list_of_parameter_value_assignments" {
                        param_extras.push(k);
                    }
                }
            }
            "module_instance" | "hierarchical_instance" => {
                let mut name = String::new();
                let mut ports = None;
                let mut has_parens = false;
                let mut extras: Vec<CstNode<'_>> = Vec::new();
                for k in c.children() {
                    match k.kind() {
                        "name_of_instance" | "simple_identifier" if name.is_empty() => {
                            name = k.text().to_string();
                        }
                        "list_of_port_connections" => ports = Some(k),
                        "(" | ")" => has_parens = true,
                        _ if k.is_named() => extras.push(k),
                        _ => {}
                    }
                }
                instances.push(Instance {
                    name,
                    ports,
                    has_parens,
                    extras,
                });
            }
            _ => {}
        }
    }
    if instances.is_empty() {
        return f.fmt_default(node);
    }
    // 全部实例都是空端口（`()`）且无额外子节点：压缩为一行（保持既有行为）
    // tree-sitter 不区分模块与接口，故统一按"无端口"判断。
    // 带注释/ERROR 内容时不压缩，改走常规多行路径（内容必须显式输出）；
    // 残余分隔符会被丢弃，不参与该判断（否则一次/二次格式化结果不同）。
    let no_extras = param_extras.iter().all(|e| is_residual_separator(*e))
        && instances
            .iter()
            .all(|i| i.extras.iter().all(|e| is_residual_separator(*e)));
    if no_extras && instances.iter().all(|i| i.ports.is_none() && i.has_parens) {
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
            &param_extras,
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
                &inst.extras,
            ));
            docs.push(Doc::Dedent);
            docs.push(Doc::Newline);
            docs.push(Doc::text(")"));
        } else if inst.has_parens {
            // 空端口 `()`：括号内的注释内联保留（`u(/* c */)`），避免丢失
            docs.push(Doc::text("("));
            let mut first = true;
            for e in &inst.extras {
                if !first {
                    docs.push(Doc::Space);
                }
                docs.push(Doc::text(e.text().trim_end().to_string()));
                first = false;
            }
            docs.push(Doc::text(")"));
        } else {
            // 无括号的实例（少见）：额外节点跟在实例名之后
            for e in &inst.extras {
                docs.push(Doc::Space);
                docs.push(Doc::text(e.text().trim_end().to_string()));
            }
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

/// "残余分隔符"ERROR 节点：仅由 `,`/`;` 组成（源文件自身的语法错误残留）。
///
/// 例如 `scheduler #(.A(1),)` 的尾随逗号——tree-sitter 将其恢复为 ERROR 节点。
/// 这类节点不表达任何语义，原样输出会让格式化结果继续无法解析，因此**不输出**。
/// 这是唯一的"丢弃内容"例外；除此之外 ERROR 恢复区内容一律原样保留。
fn is_residual_separator(node: CstNode<'_>) -> bool {
    if node.kind() != "ERROR" {
        return false;
    }
    let mut seen = false;
    for t in leaf_tokens(node) {
        let s = t.text.trim();
        if s.is_empty() {
            continue;
        }
        if !s.chars().all(|c| c == ',' || c == ';') {
            return false;
        }
        seen = true;
    }
    seen
}

/// 子树内是否含预处理指令或宏调用。
///
/// tree-sitter 会把 `` `ifdef X `` 与其后的连接合成**同一个** `named_parameter_assignment`
/// 节点（`.D(D)` 与指令同属一个节点），逐字段解析连接会丢掉指令，因此这类项必须
/// 整项原样输出。
fn subtree_has_preproc(node: CstNode<'_>) -> bool {
    if matches!(
        node.kind(),
        "conditional_compilation_directive"
            | "text_macro_usage"
            | "compiler_directive"
            | "directive"
    ) {
        return true;
    }
    node.children_iter().any(subtree_has_preproc)
}

/// 单行连接（接口压缩）：`.name(value)` 逗号空格分隔。
///
/// 只遍历赋值列表本身：`parameter_value_assignment` 下的其它子节点（注释）由
/// 调用方按位置单独输出，否则同一注释会被输出两次。
fn fmt_inline_connections(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let mut docs: Vec<Doc> = Vec::new();
    let mut first = true;
    for list in node
        .children_iter()
        .filter(|c| c.kind() == "list_of_parameter_value_assignments")
    {
        for c in list.children_iter() {
            if c.is_named() {
                if !first {
                    docs.push(Doc::Space);
                }
                if subtree_has_preproc(c) {
                    docs.push(f.fmt_default(c));
                } else {
                    docs.push(fmt_named_connection(f, c));
                }
                first = false;
            } else if c.kind() == "," {
                docs.push(Doc::text(","));
            }
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
    extras: &[CstNode<'_>],
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
    aligned_connections(f, &conns, shared_name_max, shared_value_max, 0, extras)
}

/// 端口连接（`(...)` 内，多行对齐）。
fn fmt_port_connections(
    f: &Formatter<'_>,
    node: CstNode<'_>,
    shared_name_max: usize,
    shared_value_max: usize,
    extras: &[CstNode<'_>],
) -> Doc {
    let items: Vec<CstNode<'_>> = node.children();
    let conns: Vec<CstNode<'_>> = items.iter().filter(|c| c.is_named()).copied().collect();
    aligned_connections(f, &conns, shared_name_max, shared_value_max, 0, extras)
}

/// 连接列表中的一项：连接本体，或挂在列表之外的额外节点（注释）。
struct ListItem<'t> {
    node: CstNode<'t>,
    /// 是否为连接本体（决定它是否对应 `parsed` 中的下一个元素）。
    is_conn: bool,
}

/// 连接列表中的一行。
struct RenderedLine {
    text: String,
    kind: LineKind,
    /// 该行在源码中的起始字节（注释/指令/连接实体的起点）。
    start: usize,
    /// 该行在源码中的结束字节（含追加的同行注释）。
    end: usize,
}

/// 行的种类：决定换行策略（独立注释必须独占一行，指令需顶格）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum LineKind {
    /// 连接行：`.name(value)` / `.*` / 位置连接。
    Conn,
    /// 预处理指令行（顶格输出）。
    Macro,
    /// 源中独立成行的注释。
    Comment,
}

/// 连接节点**内部**（值里）的注释文本。
///
/// 这类注释不是连接列表的子节点，`connection_parts` 不会解析它们，若不显式
/// 收集就会随值一起被丢弃（`.a ( /* c */ 1 )`）。
fn nested_comments(node: CstNode<'_>) -> Vec<String> {
    leaf_tokens(node)
        .iter()
        .filter(|t| t.is_comment)
        .map(|t| t.text.trim_end().to_string())
        .collect()
}

/// 对齐的连接列表。
///
/// `extras` 为挂在**上层的**额外节点（主要是注释）：tree-sitter 会把
/// `u(/* c */)`、`u(.a(1) /* c */)` 里的注释挂在 `hierarchical_instance` /
/// `parameter_value_assignment` 下，而不是连接列表内。它们与连接一起按源码位置
/// 排序处理，才能正确落在"行尾注释"或"独立注释行"。
fn aligned_connections(
    f: &Formatter<'_>,
    conns: &[CstNode<'_>],
    shared_name_max: usize,
    shared_value_max: usize,
    value_pad_extra: usize,
    extras: &[CstNode<'_>],
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
    // 连接 + 额外节点（注释）按源码位置合并，保证注释归属到正确的行
    let mut items: Vec<ListItem<'_>> = conns
        .iter()
        .map(|c| ListItem {
            node: *c,
            is_conn: true,
        })
        .collect();
    items.extend(extras.iter().map(|c| ListItem {
        node: *c,
        is_conn: false,
    }));
    items.sort_by_key(|i| i.node.byte_range().start);
    let mut rendered: Vec<RenderedLine> = Vec::new();
    let mut pi = 0usize;
    for item in items.iter() {
        let c = &item.node;
        if c.is_named() && c.kind().ends_with("comment") {
            let r = c.byte_range();
            // 与上一行同行 → 追加为行尾注释；否则独立成行。
            // 此前无条件追加到"最后一行"：注释位于首位（无前驱行）时整条被丢弃，
            // 位于单独一行时会被上移到前一行尾（改变注释的归属行）。
            let same_line = rendered
                .last()
                .is_some_and(|prev| !f.ws(prev.end, r.start).contains('\n'));
            if same_line {
                let prev = rendered.last_mut().expect("same_line 已确认存在前驱行");
                prev.text.push_str("  ");
                prev.text.push_str(c.text());
                prev.end = r.end;
            } else {
                rendered.push(RenderedLine {
                    text: c.text().trim_end().to_string(),
                    kind: LineKind::Comment,
                    start: r.start,
                    end: r.end,
                });
            }
            continue;
        }
        // 残余分隔符（源自身的语法错误残留，如 `#(.A(1),)` 的尾随逗号）：
        // 不输出——原样输出会让格式化结果继续无法解析。
        if !item.is_conn && is_residual_separator(*c) {
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
            rendered.push(RenderedLine {
                text: d.text().trim_end().to_string(),
                kind: LineKind::Macro,
                start: dr.start,
                end: dr.end,
            });
        }
        if pi >= parsed.len() {
            // 连接已用尽：剩余 ERROR 恢复区内容一律保留（同行则续行尾，
            // 否则独立成行）。
            if !item.is_conn {
                let same_line = rendered
                    .last()
                    .is_some_and(|prev| !f.ws(prev.end, r.start).contains('\n'));
                if same_line {
                    let prev = rendered.last_mut().expect("same_line 已确认存在前驱行");
                    prev.text.push_str(c.text().trim_end());
                    prev.end = r.end;
                } else {
                    rendered.push(RenderedLine {
                        text: c.text().trim_end().to_string(),
                        kind: LineKind::Comment,
                        start: r.start,
                        end: r.end,
                    });
                }
            }
            continue;
        }
        let conn = &parsed[pi];
        let mut line = conn_line_text(f, conn, align, name_max, value_max, value_pad_extra, inner);
        // 连接值内部的注释（CST 挂在连接节点下，不经过连接列表）：显式续在行尾，
        // 否则丢失（`.a ( /* c */ 1 )`）。拼接值已原样保留内部文本，不重复追加。
        if matches!(conn.value, ConnectionValue::Normal(_)) {
            for cmt in nested_comments(*c) {
                line.push_str("  ");
                line.push_str(&cmt);
            }
        }
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
        rendered.push(RenderedLine {
            text: line,
            kind: LineKind::Conn,
            start: dir_start,
            end: r.end,
        });
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
                    let line =
                        conn_line_text(f, c, align, name_max, value_max, value_pad_extra, inner);
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
        return wrapped_connections(f, &parsed, &items, name_max, value_max, inner);
    }
    // 输出行，行间换行/空行（单行模式用空格分隔）
    let mut prev_end: Option<usize> = None;
    let mut prev_kind: Option<LineKind> = None;
    for line in &rendered {
        if let Some(pe) = prev_end {
            // 独立注释行必须独占一行：单行模式下也不能与代码拼接，
            // 否则 `//` 注释会吞掉其后的连接内容。
            let force_break =
                line.kind == LineKind::Comment || prev_kind == Some(LineKind::Comment);
            if single_line && !force_break {
                docs.push(Doc::Space);
            } else {
                let ws = f.ws(pe, line.start);
                let blanks = count_blank_lines(ws);
                if blanks > 0 {
                    docs.push(Doc::BlankLines(blanks));
                } else {
                    docs.push(Doc::Newline);
                }
            }
        }
        match line.kind {
            // 预处理指令顶格：Dedent → 行 → Indent
            LineKind::Macro => {
                docs.push(Doc::Dedent);
                docs.push(Doc::text(line.text.clone()));
                docs.push(Doc::Indent);
            }
            _ => docs.push(Doc::text(line.text.clone())),
        }
        prev_end = Some(line.end);
        prev_kind = Some(line.kind);
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
    items: &[ListItem<'_>],
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
    let mut prev_end: Option<usize> = None;
    let mut pi = 0usize;
    for item in items.iter() {
        let c = &item.node;
        if c.is_named() && c.kind().ends_with("comment") {
            let r = c.byte_range();
            // 与前一元素同行 → 追加为该行行尾注释；否则独立成行（此前无落点时
            // 注释会被 `append_conn_comment` 静默丢弃）。
            let same_line = prev_end.is_some_and(|pe| !f.ws(pe, r.start).contains('\n'));
            if same_line {
                append_conn_comment(&mut docs, c.text());
            } else {
                if prev_end.is_some() {
                    docs.push(Doc::Newline);
                }
                docs.push(Doc::text(c.text().trim_end().to_string()));
            }
            prev_end = Some(r.end);
            continue;
        }
        if pi >= parsed.len() {
            // 连接已用尽：剩余 ERROR 恢复区内容一律保留（同行则续行尾，
            // 否则独立成行）；残余分隔符已在前面跳过。
            if !item.is_conn {
                let same_line =
                    prev_end.is_some_and(|pe| !f.ws(pe, c.byte_range().start).contains('\n'));
                if same_line {
                    append_conn_comment(&mut docs, c.text().trim_end());
                } else {
                    docs.push(Doc::Newline);
                    docs.push(Doc::text(c.text().trim_end().to_string()));
                }
                prev_end = Some(c.byte_range().end);
            }
            continue;
        }
        let conn = &parsed[pi];
        if let Some(pe) = prev_end {
            let ws = f.ws(pe, c.byte_range().start);
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
        // 连接值内部的注释：显式续在行尾（拼接值已原样保留内部文本）。
        if matches!(conn.value, ConnectionValue::Normal(_)) {
            for cmt in nested_comments(*c) {
                final_docs.push(Doc::text("  "));
                final_docs.push(Doc::text(cmt));
            }
        }
        docs.push(Doc::concat(final_docs));
        prev_end = Some(c.byte_range().end);
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
///
/// 落点可能是 `Doc::Text`（单元素连接）或 `Doc::Group`（多段连接）的最后一个
/// Text；两者都覆盖。若都匹配不上（防御性），退化为空格 + 独立注释，保证注释
/// 不会因为"找不到落点"而消失。
fn append_conn_comment(docs: &mut Vec<Doc>, comment: &str) {
    if let Some(last) = docs.last_mut() {
        match last {
            Doc::Group(children) => {
                if let Some(Doc::Text(s)) = children.last_mut() {
                    s.push_str("  ");
                    s.push_str(comment);
                    return;
                }
            }
            Doc::Text(s) => {
                s.push_str("  ");
                s.push_str(comment);
                return;
            }
            _ => {}
        }
    }
    docs.push(Doc::Space);
    docs.push(Doc::text(comment.to_string()));
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
