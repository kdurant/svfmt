//! CST 树的封装：持有 tree-sitter [`Tree`] 与源码文本。
//!
//! Formatter 层遍历时通过 [`CstNode`] 读取节点信息，源码原文始终保留，
//! 不会转换成丢失源码信息的纯 AST。

use tree_sitter::Tree;

use crate::parser::CstNode;

/// tree-sitter CST 与原始源码的绑定。
pub struct CstTree {
    tree: Tree,
    source: String,
}

impl CstTree {
    /// 用已经解析好的 tree 与源码构造 CST 树。
    pub(crate) fn new(tree: Tree, source: String) -> Self {
        CstTree { tree, source }
    }

    /// CST 根节点（`source_file`）。
    pub fn root_node(&self) -> CstNode<'_> {
        CstNode::new(self.tree.root_node(), &self.source)
    }

    /// 原始源码文本。
    #[allow(dead_code)] // Formatter 阶段使用
    pub fn source(&self) -> &str {
        &self.source
    }

    /// 整棵树是否包含任何 `ERROR` / `MISSING` 节点。
    pub fn has_error(&self) -> bool {
        self.tree.root_node().has_error()
    }

    /// 回收后的内部 tree 引用（仅供 tree-sitter API 高级用法）。
    #[allow(dead_code)]
    pub fn raw_tree(&self) -> &Tree {
        &self.tree
    }
}
