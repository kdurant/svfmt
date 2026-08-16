//! CST 树的封装：持有 tree-sitter [`Tree`] 与源码文本。
//!
//! Formatter 层遍历时通过 [`CstNode`] 读取节点信息，源码原文始终保留，
//! 不会转换成丢失源码信息的纯 AST。

use tree_sitter::Tree;

use crate::parser::CstNode;

/// tree-sitter CST 与原始源码的绑定。
///
/// 借用调用方持有的源码（`&'a str`），避免 `to_owned` 复制整份源码
/// （大文件内存翻倍）。
pub struct CstTree<'a> {
    tree: Tree,
    source: &'a str,
}

impl<'a> CstTree<'a> {
    /// 用已经解析好的 tree 与源码构造 CST 树。
    pub(crate) fn new(tree: Tree, source: &'a str) -> Self {
        CstTree { tree, source }
    }

    /// CST 根节点（`source_file`）。
    pub fn root_node(&self) -> CstNode<'_> {
        CstNode::new(self.tree.root_node(), self.source)
    }

    /// 整棵树是否包含任何 `ERROR` / `MISSING` 节点。
    pub fn has_error(&self) -> bool {
        self.tree.root_node().has_error()
    }
}
