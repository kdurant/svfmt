//! Parser 层：只负责把 SystemVerilog 源码交给 tree-sitter-systemverilog
//! 生成 CST。此处不实现任何 Formatter 规则。
//!
//! 职责：
//!   SystemVerilog source -> tree-sitter-systemverilog -> Tree-sitter CST
//!
//! 上层（Formatter）通过 `CstNode` 提供的能力遍历 CST：
//!   - root node
//!   - node kind
//!   - child nodes
//!   - named children
//!   - token text
//!   - source range
//!   - byte range
//!   - line/column
//!   - error node

use std::ops::Range;

use tree_sitter::{Language, Node, Parser, Point};

/// tree-sitter-systemverilog 解析器封装。
pub struct SvParser {
    parser: Parser,
}

/// Parser 层错误。
#[derive(Debug, thiserror::Error)]
pub enum SvParserError {
    /// 加载 SystemVerilog grammar 失败。
    #[error("无法加载 tree-sitter-systemverilog grammar")]
    GrammarLoadFailed,
    /// tree-sitter 内部解析失败。
    #[error("tree-sitter 解析失败: {0}")]
    ParseFailed(String),
}

impl SvParser {
    /// 创建一个 SystemVerilog parser。
    pub fn new() -> Result<Self, SvParserError> {
        let mut parser = Parser::new();
        let language: Language = tree_sitter_systemverilog::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|_| SvParserError::GrammarLoadFailed)?;
        Ok(SvParser { parser })
    }

    /// 解析源码，生成 CST。
    ///
    /// 语法错误不会被当作失败返回：tree-sitter 会在树中产生 `ERROR` 节点，
    /// 通过 [`CstTree::has_error`] 和 [`CstNode::is_error`] 检查即可。
    ///
    /// 返回的 [`CstTree`] 借用调用方持有的 `source`（不复制源码）。
    pub fn parse<'a>(&mut self, source: &'a str) -> Result<CstTree<'a>, SvParserError> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| SvParserError::ParseFailed("parser 未设置 language".into()))?;
        Ok(CstTree::new(tree, source))
    }
}

/// 递归收集树中所有 `ERROR` 节点。
///
/// 用于校验解析结果，方便测试和 CLI 输出错误定位。
pub fn collect_error_nodes(root: CstNode<'_>) -> Vec<CstNode<'_>> {
    let mut errors = Vec::new();
    collect_error_nodes_rec(root, &mut errors);
    errors
}

fn collect_error_nodes_rec<'tree>(node: CstNode<'tree>, out: &mut Vec<CstNode<'tree>>) {
    if node.is_error() || node.kind() == "ERROR" {
        out.push(node);
    }
    for child in node.children_iter() {
        collect_error_nodes_rec(child, out);
    }
}

/// 零分配的直接子节点迭代器（基于 [`tree_sitter::TreeCursor`]）。
///
/// 由 [`CstNode::children_iter`] 创建，顺序产出一个节点的所有直接子节点
/// （含 anonymous token），避免 `children()` 每次 collect `Vec`。
pub struct ChildrenIter<'tree> {
    cursor: tree_sitter::TreeCursor<'tree>,
    source: &'tree str,
    /// 是否已产出过首个元素。
    started: bool,
}

impl<'tree> Iterator for ChildrenIter<'tree> {
    type Item = CstNode<'tree>;
    fn next(&mut self) -> Option<CstNode<'tree>> {
        if !self.started {
            self.started = true;
        } else if !self.cursor.goto_next_sibling() {
            return None;
        }
        Some(CstNode::new(self.cursor.node(), self.source))
    }
}

/// 对一个 CST node 的便捷只读视图，携带源码文本以便读取 token text。
#[derive(Clone, Copy)]
pub struct CstNode<'tree> {
    pub(crate) inner: Node<'tree>,
    pub(crate) source: &'tree str,
}

impl<'tree> CstNode<'tree> {
    pub(crate) fn new(inner: Node<'tree>, source: &'tree str) -> Self {
        CstNode { inner, source }
    }

    /// node kind，例如 `module_declaration`、`port`。
    pub fn kind(&self) -> &'static str {
        self.inner.kind()
    }

    /// 是否是 named node（语法规则产生的节点），反之是 anonymous/`!` token。
    pub fn is_named(&self) -> bool {
        self.inner.is_named()
    }

    /// 是否是 `ERROR` 节点（语法错误产生）。
    pub fn is_error(&self) -> bool {
        self.inner.is_error()
    }

    /// 是否是 `MISSING` 节点（缺少必要 token 时产生）。
    pub fn is_missing(&self) -> bool {
        self.inner.is_missing()
    }

    /// 是否存在任何未知/错误语法。
    pub fn subtree_has_error(&self) -> bool {
        self.inner.has_error()
    }

    /// node 在源码中的 byte 范围。
    pub fn byte_range(&self) -> Range<usize> {
        self.inner.byte_range()
    }

    /// node 的起始行列。
    pub fn start_position(&self) -> Point {
        self.inner.start_position()
    }

    /// node 的结束行列。
    pub fn end_position(&self) -> Point {
        self.inner.end_position()
    }

    /// node 的原始 token 文本（不经过任何修改）。
    pub fn text(&self) -> &'tree str {
        &self.source[self.inner.byte_range()]
    }

    /// 第 `index` 个直接子节点。
    pub fn child(&self, index: usize) -> Option<CstNode<'tree>> {
        self.inner
            .child(index as u32)
            .map(|n| CstNode::new(n, self.source))
    }

    /// 第 `index` 个 named 直接子节点。
    pub fn named_child(&self, index: usize) -> Option<CstNode<'tree>> {
        self.inner
            .named_child(index as u32)
            .map(|n| CstNode::new(n, self.source))
    }

    /// 直接子节点数量（含 anonymous token）。
    pub fn child_count(&self) -> usize {
        self.inner.child_count()
    }

    /// named 直接子节点数量。
    pub fn named_child_count(&self) -> usize {
        self.inner.named_child_count()
    }

    /// 所有直接子节点（含 anonymous token，如 `;`、`,`、`(`）。
    pub fn children(&self) -> Vec<CstNode<'tree>> {
        let mut cursor = self.inner.walk();
        self.inner
            .children(&mut cursor)
            .map(|n| CstNode::new(n, self.source))
            .collect()
    }

    /// 所有直接子节点，**零分配迭代**（基于 TreeCursor）。
    ///
    /// 用于单次顺序遍历的热路径（如 `leaf_tokens`、表达式递归），
    /// 避免 `children()` 每次 collect 一个 `Vec`。需要索引/多次访问时仍用
    /// [`CstNode::children`]。
    pub fn children_iter(&self) -> ChildrenIter<'tree> {
        let mut cursor = self.inner.walk();
        let has_child = cursor.goto_first_child();
        ChildrenIter {
            cursor,
            source: self.source,
            started: !has_child,
        }
    }

    /// 所有 named 直接子节点。
    pub fn named_children(&self) -> Vec<CstNode<'tree>> {
        let mut cursor = self.inner.walk();
        self.inner
            .named_children(&mut cursor)
            .map(|n| CstNode::new(n, self.source))
            .collect()
    }

    /// 查找第一个满足 predicate 的直接 named 子节点。
    pub fn find_named_child(&self, kind: &str) -> Option<CstNode<'tree>> {
        self.named_children().into_iter().find(|c| c.kind() == kind)
    }

    /// 节点的名字（用于 `identifier`、`simple_identifier` 等），不是 `kind`。
    pub fn child_by_field_name(&self, field: &str) -> Option<CstNode<'tree>> {
        self.inner
            .child_by_field_name(field)
            .map(|n| CstNode::new(n, self.source))
    }
}

pub use tree::CstTree;

/// CST 树的 module（后续 Formatter 会用它做遍历入口）。
pub mod tree;

impl<'tree> std::fmt::Debug for CstNode<'tree> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sp = self.start_position();
        let ep = self.end_position();
        f.debug_struct("CstNode")
            .field("kind", &self.kind())
            .field("named", &self.is_named())
            .field(
                "range",
                &format!("[{}:{} - {}:{}]", sp.row, sp.column, ep.row, ep.column),
            )
            .field("text", &self.text())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> (SvParser, CstTree<'_>) {
        let mut p = SvParser::new().expect("grammar 应可加载");
        let tree = p.parse(source).expect("解析应成功");
        (p, tree)
    }

    #[test]
    fn grammar_loads() {
        SvParser::new().expect("SystemVerilog grammar 应可加载");
    }

    #[test]
    fn parse_creates_source_file_root() {
        let (_, tree) = parse("module top(); endmodule\n");
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn parse_empty_source_has_no_error() {
        let (_, tree) = parse("");
        assert!(!tree.has_error());
        assert!(tree.root_node().child_count() == 0);
    }

    #[test]
    fn module_with_ports_parses() {
        let src = "module top ( input wire clk, output reg [7:0] data ); endmodule\n";
        let (_, tree) = parse(src);
        let root = tree.root_node();
        assert!(!tree.has_error(), "不应有语法错误");

        let module = root.named_child(0).expect("应存在 module_declaration");
        assert_eq!(module.kind(), "module_declaration");
        assert!(module.is_named());
    }

    #[test]
    fn module_name_is_extracted() {
        let src = "module top ( input a ); endmodule\n";
        let (_, tree) = parse(src);
        let module = tree.root_node().named_child(0).unwrap();
        let header = module.find_named_child("module_ansi_header").unwrap();
        let name = header.child_by_field_name("name").unwrap();
        assert_eq!(name.kind(), "simple_identifier");
        assert_eq!(name.text(), "top");
    }

    #[test]
    fn module_ansi_header_structure() {
        // 注意：端口列表为空 `()` 时头部是 module_nonansi_header，
        // 带 ANSI 端口声明时才是 module_ansi_header。
        let src = "module top ( input wire clk ); endmodule\n";
        let (_, tree) = parse(src);
        let header = tree
            .root_node()
            .named_child(0)
            .unwrap()
            .find_named_child("module_ansi_header")
            .unwrap();
        let kinds: Vec<_> = header.named_children().iter().map(|c| c.kind()).collect();
        assert_eq!(
            kinds,
            [
                "module_keyword",
                "simple_identifier",
                "list_of_port_declarations"
            ]
        );
        // 第二个 named 子节点是模块名
        assert_eq!(header.named_child(1).unwrap().text(), "top");
    }

    #[test]
    fn module_nonansi_header_structure() {
        let src = "module top (); endmodule\n";
        let (_, tree) = parse(src);
        let header = tree
            .root_node()
            .named_child(0)
            .unwrap()
            .find_named_child("module_nonansi_header")
            .unwrap();
        let kinds: Vec<_> = header.named_children().iter().map(|c| c.kind()).collect();
        assert_eq!(
            kinds,
            ["module_keyword", "simple_identifier", "list_of_ports"]
        );
    }

    #[test]
    fn parameter_port_list_structure() {
        let src = "module core #(\n    parameter DATA_WIDTH = 32,\n    parameter MASK = 8'hFF\n) ( input a ); endmodule\n";
        let (_, tree) = parse(src);
        assert!(!tree.has_error(), "参数化模块不应有语法错误");

        let header = tree
            .root_node()
            .named_child(0)
            .unwrap()
            .find_named_child("module_ansi_header")
            .unwrap();
        let plist = header.find_named_child("parameter_port_list").unwrap();
        assert_eq!(
            plist.text(),
            "#(\n    parameter DATA_WIDTH = 32,\n    parameter MASK = 8'hFF\n)"
        );

        // 直接子节点：`#` `(` param1 `,` param2 `)`
        let direct: Vec<_> = plist.children().iter().map(|c| c.kind()).collect();
        assert_eq!(
            direct,
            [
                "#",
                "(",
                "parameter_port_declaration",
                ",",
                "parameter_port_declaration",
                ")"
            ]
        );

        let param_decl = plist
            .find_named_child("parameter_port_declaration")
            .unwrap();
        assert!(
            param_decl
                .find_named_child("parameter_declaration")
                .is_some()
        );
        let p = param_decl
            .find_named_child("parameter_declaration")
            .unwrap();
        assert_eq!(p.text(), "parameter DATA_WIDTH = 32");

        // parameter_declaration 内：`parameter` + list_of_param_assignments
        let p_direct: Vec<_> = p.children().iter().map(|c| c.kind()).collect();
        assert_eq!(p_direct, ["parameter", "list_of_param_assignments"]);

        // list_of_param_assignments -> param_assignment -> name = expr
        let assignments = p.find_named_child("list_of_param_assignments").unwrap();
        let assignment = assignments.find_named_child("param_assignment").unwrap();
        let a_direct: Vec<_> = assignment.children().iter().map(|c| c.kind()).collect();
        assert_eq!(
            a_direct,
            ["simple_identifier", "=", "constant_param_expression"]
        );
    }

    #[test]
    fn list_of_port_declarations_structure() {
        let src = "module top (\n    input wire clk,\n    output logic [7:0] data\n); endmodule\n";
        let (_, tree) = parse(src);
        assert!(!tree.has_error());

        let header = tree
            .root_node()
            .named_child(0)
            .unwrap()
            .find_named_child("module_ansi_header")
            .unwrap();
        let port_list = header
            .find_named_child("list_of_port_declarations")
            .unwrap();

        // 直接子节点：`(` port `,` port `)`
        let direct: Vec<_> = port_list.children().iter().map(|c| c.kind()).collect();
        assert_eq!(
            direct,
            [
                "(",
                "ansi_port_declaration",
                ",",
                "ansi_port_declaration",
                ")"
            ]
        );

        let first = port_list.find_named_child("ansi_port_declaration").unwrap();
        let port_name = first.child_by_field_name("port_name").unwrap();
        assert_eq!(port_name.text(), "clk");

        // ansi_port_declaration = net/variable_port_header + simple_identifier(端口名)
        let kinds: Vec<_> = first.named_children().iter().map(|c| c.kind()).collect();
        assert_eq!(kinds, ["net_port_header", "simple_identifier"]);
        // net_port_header = port_direction + net_port_type
        let header_kinds: Vec<_> = first
            .named_child(0)
            .unwrap()
            .named_children()
            .iter()
            .map(|c| c.kind())
            .collect();
        assert_eq!(header_kinds, ["port_direction", "net_port_type"]);
    }

    #[test]
    fn error_node_is_reported() {
        let src = "module broken ( input wire ; ; ; endmodule\n";
        let (_, tree) = parse(src);
        assert!(tree.has_error(), "错误语法应标记 has_error");
        let errors = collect_error_nodes(tree.root_node());
        assert!(!errors.is_empty(), "应能收集到 ERROR 节点");
        for e in &errors {
            assert!(e.is_error(), "ERROR 节点 is_error 应为 true");
        }
    }

    #[test]
    fn node_positions_are_correct() {
        let src = "module top();\nendmodule\n";
        let (_, tree) = parse(src);
        let module = tree.root_node().named_child(0).unwrap();
        let sp = module.start_position();
        let ep = module.end_position();
        assert_eq!((sp.row, sp.column), (0, 0));
        // endmodule 在第二行
        assert!(ep.row >= 1);
    }

    #[test]
    fn text_is_original_source_slice() {
        let src = "module top(); endmodule\n";
        let (_, tree) = parse(src);
        let module = tree.root_node().named_child(0).unwrap();
        assert_eq!(module.text(), "module top(); endmodule");
    }
}
