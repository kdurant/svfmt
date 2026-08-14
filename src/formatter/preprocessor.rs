//! 预处理器指令：原样保留。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::parser::CstNode;

/// 编译器指令节点：原样输出（保留宏、指令文本）。
pub fn fmt_directive(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let _ = f;
    Doc::text(node.text().to_string())
}
