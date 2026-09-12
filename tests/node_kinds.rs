//! 节点类型名合法性测试。
//!
//! 背景：formatter 的 dispatch 与 `is_value_kind` / `is_container_expr` 等表格
//! 全部用**字符串**比较 tree-sitter node kind。打错一个名字不会编译报错，只会
//! 静默退化为"原文输出"（design.md 明确要求"不要猜测 tree-sitter-systemverilog
//! 的 node 名称"）。
//!
//! 这里用 grammar 自带的 `node_kind_count()`/`node_kind_for_id()` 枚举全部合法
//! 类型，把这类笔误从"运行时静默退化"变成"测试失败"。

use std::collections::HashSet;

use tree_sitter::Language;

/// formatter 的全部源码（编译期嵌入，扫描字符串字面量）。
const FORMATTER_SOURCES: &[(&str, &str)] = &[
    ("mod.rs", include_str!("../src/formatter/mod.rs")),
    (
        "expressions.rs",
        include_str!("../src/formatter/expressions.rs"),
    ),
    (
        "instances.rs",
        include_str!("../src/formatter/instances.rs"),
    ),
    ("module.rs", include_str!("../src/formatter/module.rs")),
    (
        "module/params.rs",
        include_str!("../src/formatter/module/params.rs"),
    ),
    (
        "module/ports.rs",
        include_str!("../src/formatter/module/ports.rs"),
    ),
    (
        "statements.rs",
        include_str!("../src/formatter/statements.rs"),
    ),
    (
        "statements/case.rs",
        include_str!("../src/formatter/statements/case.rs"),
    ),
    (
        "statements/control.rs",
        include_str!("../src/formatter/statements/control.rs"),
    ),
    (
        "declarations.rs",
        include_str!("../src/formatter/declarations.rs"),
    ),
    ("comments.rs", include_str!("../src/formatter/comments.rs")),
    (
        "preprocessor.rs",
        include_str!("../src/formatter/preprocessor.rs"),
    ),
    ("tokens.rs", include_str!("../src/formatter/tokens.rs")),
    (
        "alignment.rs",
        include_str!("../src/formatter/alignment.rs"),
    ),
];

/// grammar 里全部合法的 node kind。
fn grammar_kinds() -> HashSet<String> {
    let lang: Language = tree_sitter_systemverilog::LANGUAGE.into();
    (0..lang.node_kind_count() as u16)
        .filter_map(|id| lang.node_kind_for_id(id))
        .map(|s| s.to_string())
        .collect()
}

/// 收集源码中的字符串字面量内容（只处理 `"..."`，忽略注释——注释里出现
/// 的类型名不参与校验；`child_by_field_name("...")` 是字段名，同样跳过）。
fn string_literals(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = src.char_indices().peekable();
    // 逐字符扫描：`//` 到行尾、`/* */` 到闭合视为注释；`"..."` 收集内容
    let bytes = src.as_bytes();
    let mut line_start = 0usize;
    while let Some((i, c)) = chars.next() {
        // 行注释
        if c == '/' && bytes.get(i + 1) == Some(&b'/') {
            for (j, d) in chars.by_ref() {
                if d == '\n' {
                    let _ = j;
                    break;
                }
            }
            continue;
        }
        // 块注释
        if c == '/' && bytes.get(i + 1) == Some(&b'*') {
            let mut prev = '\0';
            for (_, d) in chars.by_ref() {
                if prev == '*' && d == '/' {
                    break;
                }
                prev = d;
            }
            continue;
        }
        if c == '\n' {
            line_start = i + 1;
            continue;
        }
        if c == '"' {
            let mut s = String::new();
            let mut escaped = false;
            for (_, d) in chars.by_ref() {
                if escaped {
                    escaped = false;
                    s.push(d);
                    continue;
                }
                match d {
                    '\\' => escaped = true,
                    '"' => break,
                    _ => s.push(d),
                }
            }
            // 同一行里出现 `by_field_name(` 说明这是字段名，不是 node kind
            let line_prefix = &src[line_start..i];
            if !line_prefix.contains("by_field_name") {
                out.push(s);
            }
        }
    }
    out
}

/// 是否"看起来像 node kind"：全小写字母/数字/下划线（排除 `"("`、`","`
/// 这类 token 文本字面量）。
fn looks_like_kind(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && s.chars().any(|c| c.is_ascii_lowercase())
}

#[test]
fn formatter_kind_names_exist_in_grammar() {
    let kinds = grammar_kinds();
    assert!(
        kinds.len() > 100,
        "grammar 类型数量异常（{}），测试可能未正确加载 grammar",
        kinds.len()
    );

    // 允许的非 node kind 字面量：内部标记（对齐段的“行类型”、case 体风格等），
    // 它们只在 formatter 内部比较，与 grammar 无关。新增内部标记时需要显式
    // 加入本表（有意为之：避免新代码把内部标记写得像 node kind）。
    const INTERNAL_MARKERS: &[&str] = &["decl", "assign", "param", "inline", "seq", "cond"];

    let mut bad: Vec<(String, String)> = Vec::new();
    let mut checked = 0usize;
    for (file, full_src) in FORMATTER_SOURCES {
        // 只扫描生产代码：`#[cfg(test)]` 之后的单元测试里的字符串是测试数据
        // （`"clk"`、`"a + b"` 等），不是 node kind。
        let src = full_src
            .split("#[cfg(test)]")
            .next()
            .expect("split 至少产出一段");
        for lit in string_literals(src) {
            if !looks_like_kind(&lit) || INTERNAL_MARKERS.contains(&lit.as_str()) {
                continue;
            }
            checked += 1;
            let ok = kinds.contains(&lit)
                || kinds
                    .iter()
                    .any(|k| k.starts_with(lit.as_str()) || k.ends_with(lit.as_str()));
            if !ok {
                bad.push(((*file).to_string(), lit));
            }
        }
    }

    assert!(
        checked > 100,
        "扫描到的类型名字面量过少（{checked}），扫描逻辑可能失效"
    );
    assert!(
        bad.is_empty(),
        "以下字符串看起来是 node kind，但 grammar 中不存在（拼写错误会让规则静默失效）：{bad:#?}"
    );
}

/// 反向保险：dispatch 覆盖的常见结构类型确实存在于 grammar（防止 grammar 升级
/// 后类型改名而 formatter 未同步）。
#[test]
fn core_kinds_still_exist() {
    let kinds = grammar_kinds();
    for k in [
        "source_file",
        "module_declaration",
        "module_ansi_header",
        "parameter_port_list",
        "list_of_port_declarations",
        "module_instantiation",
        "hierarchical_instance",
        "list_of_port_connections",
        "data_declaration",
        "continuous_assign",
        "always_construct",
        "seq_block",
        "case_statement",
        "loop_statement",
        "block_comment",
    ] {
        assert!(kinds.contains(k), "grammar 中找不到核心类型 `{k}`");
    }
}
