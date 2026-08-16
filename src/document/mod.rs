//! Document 层：Formatter 的输出中间表示。
//!
//! CST -> Formatting Commands(Doc) -> Line Breaking -> Output
//!
//! `Doc` 由 Formatter 生成，`render` 负责把 Doc 渲染为最终文本。

use std::fmt::Write as _;

use crate::config::FormatterConfig;

/// 格式化文档元素。
#[derive(Debug, Clone)]
pub enum Doc {
    /// 原样文本（token 或原文空白）。
    Text(String),
    /// 一个空格。
    Space,
    /// 换行并输出当前缩进。
    Newline,
    /// 在已产生的换行基础上追加 `n` 个空行。
    BlankLines(usize),
    /// 缩进级别 +1（作用于之后的换行）。
    Indent,
    /// 缩进级别 -1。
    Dedent,
    /// 一组内容，其中的 [`Doc::SoftLine`] 可以在超宽时换行。
    Group(Vec<Doc>),
    /// 软换行点：组内超宽时断行，否则当作一个空格。
    SoftLine,
    /// 软换行点（无空隙）：组内超宽时断行，否则不输出任何内容。
    /// 用于"单行时无空格、超宽时换行"的位置（如函数调用 `(` 之后：
    /// `$display("...")` 单行时 `(` 后无空格）。
    SoftLineNil,
    /// 无输出。
    Nil,
}

impl Doc {
    pub fn text(s: impl Into<String>) -> Doc {
        Doc::Text(s.into())
    }
    /// 拼接一组 Doc，做组合子优化：
    /// - 过滤 [`Doc::Nil`]（无输出）
    /// - 合并相邻的 [`Doc::Text`]（减少渲染时的分段处理）
    /// - 单元素时展开为元素本身（减少一层无意义的 `Group` 嵌套）
    ///
    /// 注意：**不做内嵌 `Group` 扁平化**——`Group` 是 `column_limit` 断行的
    /// 决策单元，拍平会改变断行粒度（嵌套 `Group` 各自试排）。
    pub fn concat(docs: Vec<Doc>) -> Doc {
        let mut out: Vec<Doc> = Vec::with_capacity(docs.len());
        for d in docs {
            match d {
                Doc::Nil => {}
                Doc::Text(s) => {
                    if let Some(Doc::Text(prev)) = out.last_mut() {
                        prev.push_str(&s);
                    } else {
                        out.push(Doc::Text(s));
                    }
                }
                other => out.push(other),
            }
        }
        match out.len() {
            0 => Doc::Nil,
            1 => out.pop().expect("len==1 必有元素"),
            _ => Doc::Group(out),
        }
    }
}

/// 渲染选项。
#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub indent_width: usize,
    pub use_tab: bool,
    pub tab_width: usize,
    pub column_limit: usize,
    pub trim_trailing_whitespace: bool,
    pub max_consecutive_blank_lines: usize,
}

impl From<&FormatterConfig> for RenderOptions {
    fn from(cfg: &FormatterConfig) -> Self {
        RenderOptions {
            indent_width: cfg.indent_width as usize,
            use_tab: cfg.use_tab,
            tab_width: cfg.tab_width as usize,
            column_limit: cfg.column_limit as usize,
            trim_trailing_whitespace: cfg.trim_trailing_whitespace,
            max_consecutive_blank_lines: cfg.max_consecutive_blank_lines as usize,
        }
    }
}

/// 把 Doc 渲染为最终文本。
pub fn render(doc: &Doc, options: &RenderOptions) -> String {
    let mut renderer = Renderer {
        options,
        indent_level: 0,
        column: 0,
        out: String::new(),
        pending_newline: true,
    };
    renderer.emit(doc);
    renderer.out
}

struct Renderer<'a> {
    options: &'a RenderOptions,
    indent_level: usize,
    column: usize,
    out: String,
    /// 是否位于行首（换行后尚未输出任何非空白内容）。
    pending_newline: bool,
}

impl Renderer<'_> {
    fn emit(&mut self, doc: &Doc) {
        match doc {
            Doc::Nil => {}
            Doc::Text(s) => {
                self.write_raw(s);
            }
            Doc::Space => {
                self.write_indent();
                self.out.push(' ');
                self.column += 1;
            }
            Doc::Newline => {
                self.newline();
            }
            Doc::BlankLines(n) => {
                let n = (*n).min(self.options.max_consecutive_blank_lines);
                if self.options.trim_trailing_whitespace {
                    while self.out.ends_with(' ') || self.out.ends_with('\t') {
                        self.out.pop();
                    }
                }
                // 结束当前行，然后输出 n 个空行
                if !self.pending_newline {
                    self.out.push('\n');
                }
                for _ in 0..n {
                    self.out.push('\n');
                }
                self.pending_newline = true;
                self.column = 0;
            }
            Doc::Indent => {
                self.indent_level += 1;
            }
            Doc::Dedent => {
                self.indent_level = self.indent_level.saturating_sub(1);
            }
            Doc::Group(children) => {
                // ColumnLimit 为 0 时不启用软换行，SoftLine 视为空格。
                if self.options.column_limit == 0 {
                    for c in children {
                        self.emit(c);
                    }
                } else {
                    self.emit_group(children, 0);
                }
            }
            Doc::SoftLine => {
                self.write_indent();
                self.out.push(' ');
                self.column += 1;
            }
            Doc::SoftLineNil => {
                // flat 模式：不输出任何内容（单行时无空格）。
            }
        }
    }

    fn emit_group(&mut self, children: &[Doc], depth: usize) {
        // 组内只考虑本组范围是否超出列宽；超宽则在 SoftLine 处换行。
        let mut trial = self.clone_partial();
        for c in children {
            trial.emit(c);
        }
        let fits = trial.column <= self.options.column_limit || depth > 4;
        if fits {
            for c in children {
                self.emit(c);
            }
        } else {
            for c in children {
                match c {
                    Doc::SoftLine | Doc::SoftLineNil => self.newline(),
                    _ => self.emit(c),
                }
            }
        }
    }

    /// 复制当前状态用于试排（只关心宽度）。
    fn clone_partial(&self) -> RendererPart<'_> {
        RendererPart {
            column: self.column,
            options: self.options,
        }
    }

    fn newline(&mut self) {
        // 如果已经位于行首，不需要额外换行
        if self.pending_newline {
            return;
        }
        if self.options.trim_trailing_whitespace {
            while self.out.ends_with(' ') || self.out.ends_with('\t') {
                self.out.pop();
            }
        }
        self.out.push('\n');
        self.pending_newline = true;
        self.column = 0;
    }

    fn write_indent(&mut self) {
        if !self.pending_newline {
            return;
        }
        let width = self.indent_level * self.options.indent_width;
        if width == 0 {
            self.pending_newline = false;
            return;
        }
        if self.options.use_tab {
            let tabs = width / self.options.tab_width;
            let rem = width % self.options.tab_width;
            for _ in 0..tabs {
                self.out.push('\t');
            }
            for _ in 0..rem {
                self.out.push(' ');
            }
        } else {
            for _ in 0..width {
                self.out.push(' ');
            }
        }
        self.column += width;
        self.pending_newline = false;
    }

    fn write_raw(&mut self, s: &str) {
        // 文本可能包含换行（保留原文空白/块注释时），需要同步 column 状态。
        // 行尾空格删除（含块注释内），tab 保留。
        let mut first = true;
        for part in s.split('\n') {
            if !first {
                while self.out.ends_with(' ') {
                    self.out.pop();
                }
                self.out.push('\n');
                self.pending_newline = true;
                self.column = 0;
            }
            if !part.is_empty() {
                self.write_indent();
                self.out.push_str(part);
                self.column += display_width(part, self.options.tab_width);
                self.pending_newline = false;
            }
            first = false;
        }
    }
}

/// 试排用的轻量渲染器（只跟踪列宽）。
struct RendererPart<'a> {
    column: usize,
    options: &'a RenderOptions,
}

impl RendererPart<'_> {
    fn emit(&mut self, doc: &Doc) {
        match doc {
            Doc::Nil | Doc::Indent | Doc::Dedent | Doc::SoftLineNil => {}
            Doc::Text(s) => self.column += display_width(s, self.options.tab_width),
            Doc::Space | Doc::SoftLine => self.column += 1,
            Doc::Newline | Doc::BlankLines(_) => {
                self.column = self.options.indent_width.saturating_mul(0);
            }
            Doc::Group(children) => {
                for c in children {
                    self.emit(c);
                }
            }
        }
    }
}

/// 计算字符串在终端上的显示宽度（CJK 字符按 2 列计，tab 按 `tab_width`）。
pub fn display_width(s: &str, tab_width: usize) -> usize {
    let mut width = 0;
    for c in s.chars() {
        if c == '\t' {
            width += tab_width;
        } else if (c as u32) > 0x2e80 {
            width += 2;
        } else {
            width += 1;
        }
    }
    width
}

/// 把 Doc 序列拼接渲染。
pub fn render_sequence(docs: &[Doc], options: &RenderOptions) -> String {
    let doc = if docs.len() == 1 {
        docs[0].clone()
    } else {
        Doc::Group(docs.to_vec())
    };
    render(&doc, options)
}

/// 对齐渲染：用于对齐段 / 实例连接列宽的中间渲染，**强制单行**（column_limit=0）。
///
/// 对齐依赖单行文本来计算列宽；若 column_limit 让内部 SoftLine 断行，会得到
/// 多行字符串，破坏对齐段的结构（见 examples/hdmi.sv：断行 RHS 使后续行错乱
/// 并级联出 ERROR 节点、破坏幂等性）。
pub fn render_inline(doc: &Doc, cfg: &FormatterConfig) -> String {
    let mut opts = RenderOptions::from(cfg);
    opts.column_limit = 0;
    render(doc, &opts)
}

/// 追加文本到 String（实现 fmt::Write）。
pub fn push_to(s: &mut String, text: &str) {
    let _ = write!(s, "{text}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> RenderOptions {
        RenderOptions {
            indent_width: 4,
            use_tab: false,
            tab_width: 4,
            column_limit: 0,
            trim_trailing_whitespace: false,
            max_consecutive_blank_lines: 1,
        }
    }

    #[test]
    fn renders_text_and_space() {
        let out = render(
            &Doc::concat(vec![Doc::text("module"), Doc::Space, Doc::text("top")]),
            &opts(),
        );
        assert_eq!(out, "module top");
    }

    #[test]
    fn renders_indented_block() {
        let doc = Doc::concat(vec![
            Doc::text("begin"),
            Doc::Newline,
            Doc::Indent,
            Doc::text("a = 1;"),
            Doc::Newline,
            Doc::text("b = 2;"),
            Doc::Dedent,
            Doc::Newline,
            Doc::text("end"),
        ]);
        assert_eq!(render(&doc, &opts()), "begin\n    a = 1;\n    b = 2;\nend");
    }

    #[test]
    fn blank_lines_are_limited() {
        let doc = Doc::concat(vec![
            Doc::text("a"),
            Doc::Newline,
            Doc::BlankLines(3),
            Doc::text("b"),
        ]);
        // max_consecutive_blank_lines = 1，最多 1 个空行
        assert_eq!(render(&doc, &opts()), "a\n\nb");
    }

    #[test]
    fn soft_line_renders_as_space_when_unlimited() {
        let doc = Doc::concat(vec![Doc::text("a"), Doc::SoftLine, Doc::text("b")]);
        assert_eq!(render(&doc, &opts()), "a b");
    }

    #[test]
    fn trailing_space_is_preserved_by_default() {
        let doc = Doc::concat(vec![
            Doc::text("begin"),
            Doc::Space,
            Doc::Newline,
            Doc::text("end"),
        ]);
        assert_eq!(render(&doc, &opts()), "begin \nend");
    }
}
