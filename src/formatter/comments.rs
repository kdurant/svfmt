//! 注释处理：行尾注释对齐等。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::parser::CstNode;

/// 注释节点：原样输出。
pub fn fmt_comment(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let _ = f;
    Doc::text(node.text().to_string())
}
