//! 注释处理：注释文本归一化与输出。

use crate::document::Doc;
use crate::formatter::Formatter;
use crate::parser::CstNode;

/// 注释节点输出。
///
/// 多行块注释由 [`Formatter::comment_text`] 归一化（`*` 标记风格对齐到 `/*` 列 + 1），
/// 单行注释与普通块注释原样输出。此处只负责把归一化结果包装成 [`Doc`]。
pub fn fmt_comment(f: &Formatter<'_>, node: CstNode<'_>) -> Doc {
    Doc::text(f.comment_text(node))
}
