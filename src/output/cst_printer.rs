//! CST 递归打印器。
//!
//! 为每个 node 打印：
//!   - kind
//!   - named / unnamed
//!   - start position
//!   - end position
//!   - text（原始文本，过长截断，多行转义）
//!
//! 语法错误会产生 `ERROR` 节点，打印时会用 `!ERROR` 标记，
//! 便于快速定位非法源码片段。

use std::io::{self, Write};

use crate::parser::CstNode;

/// 打印控制选项。
#[derive(Debug, Clone)]
pub struct PrintOptions {
    /// 每一级缩进的空格数。
    pub indent_width: usize,
    /// 文本显示的最大长度（字节），超过截断并加 `...`。
    pub max_text_len: usize,
}

impl Default for PrintOptions {
    fn default() -> Self {
        PrintOptions {
            indent_width: 2,
            max_text_len: 60,
        }
    }
}

/// 递归打印以 `root` 为根的 CST。
pub fn print_cst<W: Write>(
    root: CstNode<'_>,
    out: &mut W,
    options: &PrintOptions,
) -> io::Result<()> {
    print_node(root, 0, out, options)
}

/// 把 CST 打印成字符串。
pub fn print_cst_to_string(root: CstNode<'_>, options: &PrintOptions) -> String {
    let mut buf = Vec::new();
    print_cst(root, &mut buf, options).expect("写入 Vec 不应失败");
    String::from_utf8(buf).expect("CST 打印内容应为 UTF-8")
}

fn print_node<W: Write>(
    node: CstNode<'_>,
    depth: usize,
    out: &mut W,
    options: &PrintOptions,
) -> io::Result<()> {
    let indent = " ".repeat(depth * options.indent_width);
    let sp = node.start_position();
    let ep = node.end_position();
    let name = if node.is_named() { "named" } else { "unnamed" };

    let mut flags = String::new();
    if node.is_error() {
        flags.push_str(" !ERROR");
    }
    if node.is_missing() {
        flags.push_str(" !MISSING");
    }
    if node.subtree_has_error() && !node.is_error() {
        flags.push_str(" !has_error");
    }

    let text = escape_text(node.text(), options.max_text_len);
    writeln!(
        out,
        "{}({} {}) [{row1}:{col1} - {row2}:{col2}]{flags} \"{text}\"",
        indent,
        node.kind(),
        name,
        row1 = sp.row + 1,
        col1 = sp.column,
        row2 = ep.row + 1,
        col2 = ep.column,
        flags = flags,
        text = text,
    )?;

    for child in node.children() {
        print_node(child, depth + 1, out, options)?;
    }
    Ok(())
}

/// 把原始文本转义为单行可读形式并截断。
fn escape_text(text: &str, max_len: usize) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '"' => escaped.push_str("\\\""),
            other => escaped.push(other),
        }
        if escaped.len() >= max_len {
            break;
        }
    }
    if text.len() > max_len {
        escaped.push_str("...");
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::SvParser;

    fn print(src: &str) -> String {
        let mut p = SvParser::new().unwrap();
        let tree = p.parse(src).unwrap();
        print_cst_to_string(tree.root_node(), &PrintOptions::default())
    }

    #[test]
    fn prints_kind_and_named_flag() {
        let out = print("module top(); endmodule\n");
        assert!(out.contains("(module_declaration named)"));
        assert!(out.contains("(module_keyword named)"));
    }

    #[test]
    fn prints_positions() {
        let out = print("module top();\nendmodule\n");
        // module_declaration 从第 1 行第 0 列开始
        assert!(out.contains("[1:0 - "), "out={out}");
    }

    #[test]
    fn prints_original_text() {
        let out = print("module top(); endmodule\n");
        assert!(out.contains("\"module top(); endmodule\""));
    }

    #[test]
    fn marks_error_nodes() {
        let out = print("module broken ( input wire ; ; ; endmodule\n");
        assert!(out.contains("!ERROR"), "应标记 ERROR 节点: {out}");
    }

    #[test]
    fn escapes_newlines_in_text() {
        let out = print("module top();\nendmodule\n");
        // 多行 node 的文本应把换行显示为 \n
        assert!(out.contains("\\n"));
    }

    #[test]
    fn truncates_long_text() {
        let out = print(
            "module very_long_module_name_with_many_characters_here ( input a, input b ); endmodule\n",
        );
        assert!(out.contains("..."));
    }
}
