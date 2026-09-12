//! Formatter 层：遍历 CST，生成格式化 Doc。
//!
//! 架构遵循设计文档：CST -> Formatting Commands(Doc) -> Line Breaking -> Output。
//! 模块划分见本目录各子模块。

pub mod alignment;
pub mod comments;
pub mod declarations;
pub mod expressions;
pub mod instances;
pub mod module;
pub mod preprocessor;
pub mod statements;
pub mod tokens;

use crate::config::FormatterConfig;
use crate::document::{Doc, RenderOptions, render};
use crate::parser::{CstNode, SvParser, collect_problem_nodes};

/// 格式化器：持有配置与源码。
pub struct Formatter<'a> {
    pub cfg: &'a FormatterConfig,
    pub src: &'a str,
}

/// 是否启用 SVDBG 调试输出。
///
/// 用 `OnceLock` 缓存环境变量查询结果：SVDBG 在热路径被大量判断，
/// 直接 `env::var` 会在每个节点都触发系统调用，改为全局只查询一次。
pub fn svdbg() -> bool {
    static SVDBG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SVDBG.get_or_init(|| std::env::var_os("SVDBG").is_some())
}

impl<'a> Formatter<'a> {
    pub fn new(cfg: &'a FormatterConfig, src: &'a str) -> Self {
        Formatter { cfg, src }
    }

    /// 是否启用 SVDBG 调试输出。
    pub fn svdbg(&self) -> bool {
        svdbg()
    }

    /// 解析并格式化源码。
    pub fn format_source(src: &str, cfg: &FormatterConfig) -> Result<String, FormatterError> {
        Ok(Self::format_source_checked(src, cfg)?.0)
    }

    /// 解析并格式化源码，返回输出与语法问题节点数（`ERROR` + `MISSING`）。
    ///
    /// 语法错误不会使格式化失败（tree-sitter 通过 ERROR/MISSING 节点恢复），
    /// 但调用方可用问题数决定是否告警或以非零状态退出（`--fail-on-parse-error`）。
    /// 计数范围与 `CstTree::has_error()` 保持一致。
    pub fn format_source_checked(
        src: &str,
        cfg: &FormatterConfig,
    ) -> Result<(String, usize), FormatterError> {
        let mut parser = SvParser::new()?;
        let tree = parser.parse(src)?;
        let error_count = collect_problem_nodes(tree.root_node()).len();
        let formatter = Formatter::new(cfg, src);
        let doc = formatter.fmt(tree.root_node());
        let options = RenderOptions::from(cfg);
        Ok((render(&doc, &options), error_count))
    }

    // ---------- 基础工具 ----------

    pub fn text(&self, s: impl Into<String>) -> Doc {
        Doc::text(s)
    }

    pub fn spc(&self) -> Doc {
        Doc::Space
    }

    pub fn nl(&self) -> Doc {
        Doc::Newline
    }

    pub fn blank(&self, n: usize) -> Doc {
        Doc::BlankLines(n)
    }

    /// 节点原文。
    pub fn raw(&self, node: CstNode<'_>) -> Doc {
        Doc::text(node.text())
    }

    /// 注释输出。
    ///
    /// 多行块注释（`/* ... */`）：`*` 标记风格（如 `/*\n * ...\n */`）的内部行
    /// 对齐到注释起始列 + 1（剥掉公共前导空白后补 1 空格）；普通文本注释原样保留。
    /// 单行注释原样输出。
    pub fn fmt_comment(&self, node: CstNode<'_>) -> Doc {
        Doc::text(self.comment_text(node))
    }

    /// 归一化注释文本。
    ///
    /// 多行块注释（`/* ... */`）：`*` 标记风格（如 `/*\n * ...\n */`）的内部 `*`
    /// 行对齐到注释起始列 + 1（剥掉公共前导空白后补 1 空格），普通文本注释保留
    /// 内部行原样（不含起始列空白）；单行注释原样返回。
    pub fn comment_text(&self, node: CstNode<'_>) -> String {
        let text = node.text();
        let is_block = text.contains("/*") && text.contains("*/") && text.contains('\n');
        if !is_block {
            return text.to_string();
        }
        let lines: Vec<&str> = text.split('\n').collect();
        // 内部行（跳过首行）的公共前导空白数
        let common = lines
            .iter()
            .skip(1)
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);
        // 是否为 `*` 标记风格：首个内部行剥掉公共缩进后以 `*` 开头
        let star_style = lines
            .get(1)
            .map(|l| l[common.min(l.len())..].trim_start().starts_with('*'))
            .unwrap_or(false);
        let mut out = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
                let stripped = &line[common.min(line.len())..];
                if star_style && stripped.trim_start().starts_with('*') {
                    // `*` 标记行：补 1 空格使 `*` 对齐到 `/*` 列 + 1
                    out.push(' ');
                    out.push_str(stripped.trim_start());
                } else {
                    out.push_str(stripped);
                }
            } else {
                out.push_str(line);
            }
        }
        out
    }

    /// 原文空白：从 `start` 到 `end`。
    pub fn ws(&self, start: usize, end: usize) -> &str {
        if end <= start || start >= self.src.len() {
            return "";
        }
        &self.src[start..end.min(self.src.len())]
    }

    /// 换行前的行内空白（保留尾随空格）。
    pub fn trailing_inline<'b>(&self, ws: &'b str) -> &'b str {
        if let Some(pos) = ws.find('\n') {
            &ws[..pos]
        } else {
            ws
        }
    }

    /// 换行后的行内空白（用于行内对齐时保留）。
    pub fn leading_inline<'b>(&self, ws: &'b str) -> &'b str {
        if let Some(pos) = ws.rfind('\n') {
            &ws[pos + 1..]
        } else {
            ws
        }
    }

    // ---------- dispatch ----------

    pub fn fmt(&self, node: CstNode<'_>) -> Doc {
        if self.svdbg() {
            eprintln!("[fmt] kind={}", node.kind());
        }
        match node.kind() {
            "source_file" => self.fmt_source_file(node),
            "module_declaration" => module::fmt_module_declaration(self, node),
            "module_ansi_header" | "module_nonansi_header" => module::fmt_module_header(self, node),
            "parameter_port_list" => module::fmt_parameter_port_list(self, node),
            "list_of_port_declarations" | "list_of_ports" => module::fmt_port_list(self, node),
            "data_declaration" | "net_declaration" => {
                declarations::fmt_data_declaration(self, node)
            }
            "parameter_declaration" | "local_parameter_declaration" => {
                declarations::fmt_parameter_declaration(self, node)
            }
            "parameter_override" => self.raw(node),
            "continuous_assign" => declarations::fmt_continuous_assign(self, node),
            "always_construct" | "initial_construct" | "final_construct" => {
                statements::fmt_procedural_construct(self, node)
            }
            "task_declaration" | "function_declaration" => {
                statements::fmt_task_or_function_declaration(self, node)
            }
            "seq_block" => statements::fmt_seq_block(self, node),
            "par_block" => statements::fmt_par_block(self, node),
            "conditional_statement" => statements::fmt_conditional(self, node),
            "case_statement" => statements::fmt_case_statement(self, node),
            "case_generate_construct" => statements::fmt_case_generate_construct(self, node),
            "case_item" => statements::fmt_case_item(self, node, &[]),
            "simple_immediate_assert_statement" => statements::fmt_assert(self, node),
            "loop_statement" => statements::fmt_loop_statement(self, node),
            "generate_region" | "generate_block" => statements::fmt_generate(self, node),
            "loop_generate_construct" => statements::fmt_loop_statement(self, node),
            "conditional_generate_construct" => {
                if let Some(child) = node.named_child(0) {
                    match child.kind() {
                        "case_generate_construct" => {
                            statements::fmt_case_generate_construct(self, child)
                        }
                        "if_generate_construct" => {
                            statements::fmt_if_generate_construct(self, child)
                        }
                        _ => statements::fmt_loop_statement(self, node),
                    }
                } else {
                    statements::fmt_loop_statement(self, node)
                }
            }
            "statement_or_null" | "statement" | "statement_item" => self.fmt_statement(node),
            "module_item" => {
                if let Some(child) = node.named_child(0) {
                    self.fmt(child)
                } else {
                    self.raw(node)
                }
            }
            "module_instantiation" => instances::fmt_module_instantiation(self, node),
            "parameter_value_assignment" | "list_of_port_connections" => self.raw(node),
            "typedef_declaration" => declarations::fmt_typedef(self, node),
            "genvar_declaration" => self.fmt_default(node),
            "attribute_instance" => self.raw(node),
            "blocking_assignment"
            | "nonblocking_assignment"
            | "operator_assignment"
            | "assignment_expression"
            | "variable_assignment"
            | "net_assignment" => {
                expressions::fmt_expr(self, node, &expressions::ExprCtx::default())
            }
            // 调用类节点：经 fmt_expr/fmt_call 处理（支持 column_limit 断行，
            // 并规范化参数空格）。此前这些节点走 raw 原文输出。
            "subroutine_call_statement" | "subroutine_call" | "system_tf_call" | "tf_call" => {
                expressions::fmt_expr(self, node, &expressions::ExprCtx::default())
            }
            _ if node.kind().ends_with("comment") => comments::fmt_comment(self, node),
            _ if node.kind().ends_with("compiler_directive")
                || node.kind().ends_with("directive") =>
            {
                preprocessor::fmt_directive(self, node)
            }
            _ if expressions::is_value_kind(node.kind()) => expressions::dispatch_expr(self, node),
            _ => self.fmt_default(node),
        }
    }

    /// 顶层 source_file：逐项格式化，保留空行。
    fn fmt_source_file(&self, node: CstNode<'_>) -> Doc {
        let mut docs: Vec<Doc> = Vec::new();
        let mut prev_end: Option<usize> = None;
        for child in node.children_iter() {
            let start = child.byte_range().start;
            if let Some(pe) = prev_end {
                let ws = self.ws(pe, start);
                let blank_lines = count_blank_lines(ws);
                if blank_lines > 0 {
                    docs.push(self.blank(blank_lines));
                } else if ws.contains('\n') {
                    docs.push(Doc::Newline);
                } else if !ws.is_empty() {
                    docs.push(self.text(ws.to_string()));
                }
            }
            let f = self.fmt(child);
            if !matches!(f, Doc::Nil) {
                docs.push(f);
            }
            // 记录"内容末尾"（不含行尾换行），使空白计算包含整行节点的换行。
            // 不能用 `rfind('\n')`：对不以换行结尾的多行节点（如
            // `module a;\nendmodule`）会取到"最后一行之前的换行"，使 prev_end
            // 指向节点内部，导致与下一节点之间的空白计算把最后一行内容也混入，
            // 插入多余空行 / 拆散同行注释。
            let end = child.byte_range().end;
            let content_end = if child.text().ends_with('\n') {
                end.saturating_sub(1)
            } else {
                end
            };
            prev_end = Some(content_end);
        }
        // 文件末尾换行
        docs.push(Doc::Newline);
        Doc::concat(docs)
    }

    /// 默认格式化：输出节点原文。
    ///
    /// 多行节点会先按节点起始列剥掉内部行的缩进，再交给渲染器按当前缩进重排。
    /// 否则原文里的源缩进会与当前缩进叠加，使多次格式化时缩进逐轮累积
    /// （幂等性破坏，如 `randcase` 等未结构化节点）。
    pub fn fmt_default(&self, node: CstNode<'_>) -> Doc {
        let text = node.text();
        if !text.contains('\n') {
            return Doc::text(text);
        }
        Doc::text(strip_indent(text, node.start_position().column))
    }

    /// 语句包装节点（statement_or_null/statement/statement_item）：
    /// 解开包装取实际语句，并补回语句末尾的 `;`。
    fn fmt_statement(&self, node: CstNode<'_>) -> Doc {
        let inner = statements::unwrap_statement(self, node);
        // unwrap 未取得进展（如空语句 `;` 的 statement_or_null，仅含 unnamed 子节点）
        // 时原样输出，避免 self.fmt(inner) 无限递归（栈溢出）。
        if inner.kind() == node.kind()
            && inner.byte_range().start == node.byte_range().start
            && inner.byte_range().end == node.byte_range().end
        {
            return self.raw(node);
        }
        let mut doc = self.fmt(inner);
        // 结构语句（if/case/for/begin 等）自身不带 `;`
        let no_semi = matches!(
            inner.kind(),
            "conditional_statement" | "case_statement" | "loop_statement" | "seq_block"
        );
        // 若包装节点包含 `;` 而实际语句 Doc 未输出，则补上
        let text = node.text();
        let trimmed = text.trim_end();
        if !no_semi && trimmed.ends_with(';') && !ends_with_semi(&doc) {
            doc = Doc::concat(vec![doc, Doc::text(";")]);
        }
        doc
    }

    /// 当前缩进是否生效（模块体不缩进时由上层控制）。
    pub fn module_indent(&self) -> Doc {
        if self.cfg.indent_module_contents {
            Doc::Indent
        } else {
            Doc::Nil
        }
    }
}

/// 统计空白中的空行数（连续换行数量减去 1）。
pub fn count_blank_lines(ws: &str) -> usize {
    let nl = ws.matches('\n').count();
    nl.saturating_sub(1)
}

/// 剥掉多行文本内部行（第 2 行起）的前导空白，最多剥 `strip` 列，
/// 且不超过内部行的公共前导空白。
///
/// 首行原样保留；全空行不参与统计。用于原文输出路径：内部行既保留原有相对
/// 缩进（不会因过度剥离而丢结构），又避免"源缩进 + 当前缩进"叠加导致多次
/// 格式化时缩进累积。
pub fn strip_indent(text: &str, strip: usize) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() <= 1 || strip == 0 {
        return text.to_string();
    }
    let common = lines
        .iter()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    let strip = strip.min(common);
    if strip == 0 {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
            out.push_str(&line[strip.min(line.len())..]);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// 判断 Doc 是否以 `;` 结尾（逆序遍历找到最后一个 Text）。
fn ends_with_semi(doc: &Doc) -> bool {
    fn last_text(doc: &Doc) -> Option<&str> {
        match doc {
            Doc::Text(s) => Some(s),
            // 逆序查找最后一个 Text：跳过末尾的 Indent/Dedent/Newline 等
            // 非文本元素（如 fmt_assert 的结构化输出以 Dedent 结尾）。
            Doc::Group(children) => children.iter().rev().find_map(last_text),
            Doc::Fill(children) => children.iter().rev().find_map(last_text),
            _ => None,
        }
    }
    last_text(doc).is_some_and(|s| s.trim_end().ends_with(';'))
}

/// Formatter 错误。
#[derive(Debug, thiserror::Error)]
pub enum FormatterError {
    #[error(transparent)]
    Parse(#[from] crate::parser::SvParserError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        let cfg = FormatterConfig::default();
        Formatter::format_source(src, &cfg).unwrap()
    }

    #[test]
    fn empty_source_is_empty() {
        assert_eq!(fmt(""), "");
    }

    #[test]
    fn simple_module_roundtrip() {
        let src = "module top();\nendmodule\n";
        let out = fmt(src);
        assert!(out.contains("module top();"));
        assert!(out.contains("endmodule"));
    }

    #[test]
    fn idempotent_on_simple_module() {
        let src = "module top();\nendmodule\n";
        let once = fmt(src);
        let twice = fmt(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn wildcard_port_connection_is_preserved() {
        // `.*` 通配端口连接：曾因被判为"无名连接"而整条过滤，丢失连接语义，
        // 并导致 ram_sdp.sv 二次格式化结构漂移（不幂等）。
        let src = "module t;\nfoo u(\n.*\n);\nendmodule\n";
        let out = fmt(src);
        assert!(out.contains(".*"), "通配端口连接被丢弃: {out:?}");
        assert_eq!(out, fmt(&out), "通配端口连接应幂等");
    }

    #[test]
    fn multi_assignment_and_inc_dec_statements_are_preserved() {
        // 多赋值 assign 曾只保留最后一条；inc/dec 语句（无 rhs）曾在对齐段被整条丢弃。
        let src =
            "module t;\nassign a = 1, b = 2;\ninitial begin\n  c++;\n  d = 3;\nend\nendmodule\n";
        let out = fmt(src);
        assert!(
            out.contains("assign a = 1, b = 2;"),
            "多赋值 assign 丢语句: {out:?}"
        );
        assert!(out.contains("c++;"), "inc/dec 语句被丢弃: {out:?}");
        assert_eq!(out, fmt(&out), "应幂等");
    }

    #[test]
    fn multi_param_port_list_is_preserved() {
        // `parameter A = 1, B = 2` 曾只保留第一条赋值。
        let src = "module m #( parameter A = 1, B = 2 )\n( input logic clk );\nendmodule\n";
        let out = fmt(src);
        assert!(
            out.contains("parameter A = 1, B = 2"),
            "多参数赋值被丢弃: {out:?}"
        );
        assert_eq!(out, fmt(&out), "应幂等");
    }

    #[test]
    fn trailing_comments_align_by_display_width_with_cjk() {
        // 列宽必须按显示宽度（CJK=2）计算：含中文的行与 ASCII 行的行尾注释应落在同一列。
        let src = "module t;\ninitial begin\n  a = \"中文中文\"; // 中\n  bbbbb = \"aaaa\"; // x\nend\nendmodule\n";
        let out = fmt(src);
        let cols: Vec<usize> = out
            .lines()
            .filter(|l| l.contains("//"))
            .map(|l| {
                let i = l.find("//").expect("应能定位注释");
                crate::document::display_width(&l[..i], 4)
            })
            .collect();
        assert_eq!(cols.len(), 2, "应有两行行尾注释: {out}");
        assert_eq!(cols[0], cols[1], "行尾注释应按显示列对齐: {out}");
    }

    #[test]
    fn macro_params_in_port_list_are_preserved() {
        // 裸 `list_of_param_assignments`（宏参数）曾被整体忽略，模块参数静默丢失。
        let src = "module m #(`INV(a), `INV(b))\n( input logic clk );\nendmodule\n";
        let out = fmt(src);
        assert!(out.contains("`INV(a),"), "宏参数被丢弃: {out:?}");
        assert!(out.contains("`INV(b)"), "宏参数被丢弃: {out:?}");
        assert_eq!(out, fmt(&out), "应幂等");
    }

    #[test]
    fn module_body_import_is_preserved() {
        // import 在模块体内被解析为 data_declaration(package_import_declaration)，
        // 不应被当作可对齐声明丢弃（回归：曾输出为孤立的 `;`）。
        let src = "module tb;\n\n  import sata_pkg::*;\n\n  int errors = 0;\nendmodule\n";
        let out = fmt(src);
        assert!(
            out.contains("import sata_pkg::*;"),
            "import 被丢弃: {out:?}"
        );
        assert!(!out.contains(" ;"), "import 被格式化为孤立分号: {out:?}");
        // 幂等
        assert_eq!(out, fmt(&out));
    }
}
