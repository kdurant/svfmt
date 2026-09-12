//! 预处理器指令（`` `include ``、`` `define ``、`` `ifdef/`else/`endif `` 等）。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::parser::CstNode;

/// 编译器指令节点输出：文本原样保留（不改动宏与指令内容）。
///
/// `directives_at_line_start` 为 true 时在指令前加 [`Doc::Col0`]，使指令顶格输出
/// （不随当前缩进层级）。
pub fn fmt_directive(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    let text = Doc::text(node.text());
    if f.cfg.directives_at_line_start {
        Doc::concat(vec![Doc::Col0, text])
    } else {
        text
    }
}
