//! CLI 层：解析命令行参数。

use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};

use crate::config::{FormatterConfig, ModuleConfig, ReformatCase, SpaceConfig};

/// SystemVerilog Formatter。
#[derive(Debug, Parser)]
#[command(
    name = "svfmt",
    version,
    about = "SystemVerilog Formatter：基于 tree-sitter-systemverilog 的源码格式化工具",
    propagate_version = true
)]
pub struct Cli {
    /// 输入 .sv 文件路径（可多个，支持 glob）；省略或为 `-` 时从标准输入读取
    #[arg(value_name = "FILE")]
    pub files: Vec<PathBuf>,

    /// 输出文件路径（默认输出到 stdout）
    #[arg(short = 'o', long, value_name = "OUT")]
    pub output: Option<PathBuf>,

    /// 就地格式化（覆盖输入文件）
    #[arg(long, action = ArgAction::SetTrue)]
    pub in_place: bool,

    /// 配置文件路径（YAML 或 TOML）
    #[arg(long, value_name = "CONFIG")]
    pub config: Option<PathBuf>,

    /// 打印默认配置并退出
    #[arg(long, action = ArgAction::SetTrue)]
    pub dump_config: bool,

    /// 解析并打印 CST（调试用）
    #[arg(long, action = ArgAction::SetTrue)]
    pub cst: bool,

    /// 源码存在语法错误时仍格式化（默认）；开启后以非零状态退出
    #[arg(long, action = ArgAction::SetTrue)]
    pub fail_on_parse_error: bool,

    /// 命令行格式覆盖项（优先级：命令行 > 配置文件 > 默认值）
    #[command(flatten)]
    pub overrides: ConfigArgs,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// 子命令。
#[derive(Debug, Subcommand)]
pub enum Command {
    /// 解析 SystemVerilog 文件并递归打印 CST
    Cst(CstArgs),
}

/// `cst` 子命令参数。
#[derive(Debug, Args)]
pub struct CstArgs {
    /// 要解析的 .sv 文件
    #[arg(value_name = "FILE")]
    pub file: PathBuf,

    /// CST 打印时每一级缩进的空格数
    #[arg(long, default_value_t = 2, value_name = "N")]
    pub indent_width: usize,

    /// CST 打印时节点文本显示的最大长度（字节），超过截断
    #[arg(long, default_value_t = 60, value_name = "N")]
    pub max_text_len: usize,

    /// 解析到语法错误时仍打印 CST，但以非零状态退出
    #[arg(long, action = ArgAction::SetTrue)]
    pub fail_on_error: bool,
}

/// 命令行格式覆盖项。所有字段为 `Option`：未指定时保留配置文件/默认值。
///
/// 优先级：命令行 > 配置文件 > 默认值。与 `FormatterConfig` 字段一一对应并平铺，
/// 命名与配置键一致（如 `--column-limit`、`--after-colon`、`--port-alignment`）。
#[derive(Debug, Default, Args)]
pub struct ConfigArgs {
    /// 每一级代码块的缩进空格数
    #[arg(long, value_name = "N")]
    pub indent_width: Option<u32>,
    /// 模块内部的第一级代码是否缩进一个 `indent_width`
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub indent_module_contents: Option<bool>,
    /// 是否使用 Tab 进行缩进
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub use_tab: Option<bool>,
    /// 一个 Tab 显示的宽度
    #[arg(long, value_name = "N")]
    pub tab_width: Option<u32>,
    /// 每行最大列数，0 表示不限制
    #[arg(long, value_name = "N")]
    pub column_limit: Option<u32>,
    /// 是否删除行尾多余空格与制表符
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub trim_trailing_whitespace: Option<bool>,

    /// 空格相关配置
    #[command(flatten)]
    pub space: SpaceArgs,

    /// 连续空行的最大数量
    #[arg(long, value_name = "N")]
    pub max_consecutive_blank_lines: Option<u32>,
    /// 两个 always/initial/function/task 块之间是否至少保留一个空行
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub blank_line_between_procedures: Option<bool>,
    /// 行尾注释是否按列对齐
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub align_trailing_comments: Option<bool>,
    /// 注释与代码之间的最少空格数
    #[arg(long, value_name = "N")]
    pub comment_indent: Option<u32>,
    /// 行尾注释对齐到的列号
    #[arg(long, value_name = "N")]
    pub comment_column: Option<u32>,
    /// 同一连续块内的 `=`/`<=` 是否对齐
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub align_assignments: Option<bool>,
    /// 实例化的端口连接是否按左右括号对齐
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub align_instance_ports: Option<bool>,
    /// 实例端口连接括号内侧保留空格数
    #[arg(long, value_name = "N")]
    pub space_inside_instance_port_parens: Option<u32>,
    /// case 语句中冒号前的表达式是否对齐
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub align_case_items: Option<bool>,

    /// 模块相关配置
    #[command(flatten)]
    pub module: ModuleArgs,

    /// 实例化端口超过多少个时强制换行（0 不强制）
    #[arg(long, value_name = "N")]
    pub wrap_instance_ports: Option<u32>,
    /// `begin` 是否另起一行
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub begin_end_on_newline: Option<bool>,
    /// `begin` 后是否紧跟第一条语句
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub end_of_line_for_begin: Option<bool>,
    /// `end` 是否必须单独占一行
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub end_on_newline: Option<bool>,
    /// `else` 是否另起一行
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub else_on_newline: Option<bool>,
    /// 延时控制语句是否可与相邻语句放在同一行
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub delay_on_same_line: Option<bool>,
    /// 预处理器指令（`ifdef/`else/`define 等）是否从行首开始（不缩进）
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub directives_at_line_start: Option<bool>,
    /// 是否统一 case/casez/casex 风格（none|casez|casex）
    #[arg(long)]
    pub reformat_case: Option<ReformatCase>,
    /// case 分支内容相对 case 的缩进层级
    #[arg(long, value_name = "N")]
    pub case_indent_level: Option<u32>,
}

/// 空格相关覆盖项（`--space-*`）。
#[derive(Debug, Default, Args)]
pub struct SpaceArgs {
    /// 二元运算符两侧是否加空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub around_binary_operator: Option<bool>,
    /// 逗号后是否加一个空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub after_comma: Option<bool>,
    /// `for` 循环与实例端口连接中的分号后是否加空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub after_semicolon: Option<bool>,
    /// 函数/任务调用时，函数名与左括号之间是否加空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub before_parens_in_function_call: Option<bool>,
    /// 控制语句与左括号之间是否加空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub before_control_statement_parens: Option<bool>,
    /// 圆括号内侧是否加空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub inside_parens: Option<bool>,
    /// 赋值符号两侧是否加空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub around_assignment: Option<bool>,
    /// 冒号前是否加空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub before_colon: Option<bool>,
    /// 冒号后是否加空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub after_colon: Option<bool>,
    /// 一元运算符后是否加空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub after_unary_operators: Option<bool>,
    /// `@` 之后、左括号之前是否加空格
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub after_at: Option<bool>,
}

/// 模块相关覆盖项（`--module-*`）。
#[derive(Debug, Default, Args)]
pub struct ModuleArgs {
    /// 模块参数列表的左括号 `(` 是否另起一行
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub parameter_list_break_before_open_paren: Option<bool>,
    /// 模块端口列表的左括号 `(` 是否另起一行
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub port_list_break_before_open_paren: Option<bool>,
    /// 实例端口列表的左括号 `(` 是否另起一行
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub instance_port_list_break_before_open_paren: Option<bool>,
    /// 参数声明中的类型、名称、默认值是否按列对齐
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub align_parameters: Option<bool>,
    /// 每个端口是否单独占一行
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub newline_per_port: Option<bool>,
    /// 端口列表中方向关键字与端口名称是否对齐
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub port_alignment: Option<bool>,
    /// 实例化时每个端口连接是否单独占一行
    #[arg(long, require_equals = true, num_args(0..=1), default_missing_value = "true")]
    pub newline_per_instance_port: Option<bool>,
}

impl ConfigArgs {
    /// 将命令行覆盖项合并到配置上（仅覆盖显式指定的项）。
    pub fn apply_to(&self, cfg: &mut FormatterConfig) {
        if let Some(v) = self.indent_width {
            cfg.indent_width = v;
        }
        if let Some(v) = self.indent_module_contents {
            cfg.indent_module_contents = v;
        }
        if let Some(v) = self.use_tab {
            cfg.use_tab = v;
        }
        if let Some(v) = self.tab_width {
            cfg.tab_width = v;
        }
        if let Some(v) = self.column_limit {
            cfg.column_limit = v;
        }
        if let Some(v) = self.trim_trailing_whitespace {
            cfg.trim_trailing_whitespace = v;
        }
        self.space.apply_to(&mut cfg.space);
        if let Some(v) = self.max_consecutive_blank_lines {
            cfg.max_consecutive_blank_lines = v;
        }
        if let Some(v) = self.blank_line_between_procedures {
            cfg.blank_line_between_procedures = v;
        }
        if let Some(v) = self.align_trailing_comments {
            cfg.align_trailing_comments = v;
        }
        if let Some(v) = self.comment_indent {
            cfg.comment_indent = v;
        }
        if let Some(v) = self.comment_column {
            cfg.comment_column = v;
        }
        if let Some(v) = self.align_assignments {
            cfg.align_assignments = v;
        }
        if let Some(v) = self.align_instance_ports {
            cfg.align_instance_ports = v;
        }
        if let Some(v) = self.space_inside_instance_port_parens {
            cfg.space_inside_instance_port_parens = v;
        }
        if let Some(v) = self.align_case_items {
            cfg.align_case_items = v;
        }
        self.module.apply_to(&mut cfg.module);
        if let Some(v) = self.wrap_instance_ports {
            cfg.wrap_instance_ports = v;
        }
        if let Some(v) = self.begin_end_on_newline {
            cfg.begin_end_on_newline = v;
        }
        if let Some(v) = self.end_of_line_for_begin {
            cfg.end_of_line_for_begin = v;
        }
        if let Some(v) = self.end_on_newline {
            cfg.end_on_newline = v;
        }
        if let Some(v) = self.else_on_newline {
            cfg.else_on_newline = v;
        }
        if let Some(v) = self.delay_on_same_line {
            cfg.delay_on_same_line = v;
        }
        if let Some(v) = self.directives_at_line_start {
            cfg.directives_at_line_start = v;
        }
        if let Some(v) = self.reformat_case {
            cfg.reformat_case = v;
        }
        if let Some(v) = self.case_indent_level {
            cfg.case_indent_level = v;
        }
    }
}

impl SpaceArgs {
    fn apply_to(&self, cfg: &mut SpaceConfig) {
        if let Some(v) = self.around_binary_operator {
            cfg.around_binary_operator = v;
        }
        if let Some(v) = self.after_comma {
            cfg.after_comma = v;
        }
        if let Some(v) = self.after_semicolon {
            cfg.after_semicolon = v;
        }
        if let Some(v) = self.before_parens_in_function_call {
            cfg.before_parens_in_function_call = v;
        }
        if let Some(v) = self.before_control_statement_parens {
            cfg.before_control_statement_parens = v;
        }
        if let Some(v) = self.inside_parens {
            cfg.inside_parens = v;
        }
        if let Some(v) = self.around_assignment {
            cfg.around_assignment = v;
        }
        if let Some(v) = self.before_colon {
            cfg.before_colon = v;
        }
        if let Some(v) = self.after_colon {
            cfg.after_colon = v;
        }
        if let Some(v) = self.after_unary_operators {
            cfg.after_unary_operators = v;
        }
        if let Some(v) = self.after_at {
            cfg.after_at = v;
        }
    }
}

impl ModuleArgs {
    fn apply_to(&self, cfg: &mut ModuleConfig) {
        if let Some(v) = self.parameter_list_break_before_open_paren {
            cfg.parameter_list_break_before_open_paren = v;
        }
        if let Some(v) = self.port_list_break_before_open_paren {
            cfg.port_list_break_before_open_paren = v;
        }
        if let Some(v) = self.instance_port_list_break_before_open_paren {
            cfg.instance_port_list_break_before_open_paren = v;
        }
        if let Some(v) = self.align_parameters {
            cfg.align_parameters = v;
        }
        if let Some(v) = self.newline_per_port {
            cfg.newline_per_port = v;
        }
        if let Some(v) = self.port_alignment {
            cfg.port_alignment = v;
        }
        if let Some(v) = self.newline_per_instance_port {
            cfg.newline_per_instance_port = v;
        }
    }
}
