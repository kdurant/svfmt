//! 配置层：Formatter 的所有行为都由 `FormatterConfig` 控制。
//!
//! 配置项定义与默认值参考 `verilog_format.md`。
//! 支持 TOML 配置文件加载，并支持 `--dump-config` 导出默认配置。

use std::path::Path;

use serde::{Deserialize, Serialize};

/// case 风格统一选项。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lowercase")]
#[derive(Default)]
pub enum ReformatCase {
    /// 不修改 case 关键字。
    #[default]
    None,
    /// 统一为 `casez`。
    Casez,
    /// 统一为 `casex`。
    Casex,
}

/// 空格相关配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceConfig {
    /// 二元运算符两侧是否加空格。
    #[serde(default = "default_true")]
    pub around_binary_operator: bool,
    /// 逗号后是否加一个空格。
    #[serde(default = "default_true")]
    pub after_comma: bool,
    /// `for` 循环与实例端口连接中的分号后是否加空格。
    #[serde(default = "default_true")]
    pub after_semicolon: bool,
    /// 函数/任务调用时，函数名与左括号之间是否加空格。
    #[serde(default = "default_false")]
    pub before_parens_in_function_call: bool,
    /// 控制语句与左括号之间是否加空格。
    #[serde(default = "default_false")]
    pub before_control_statement_parens: bool,
    /// 圆括号内侧是否加空格。
    #[serde(default = "default_false")]
    pub inside_parens: bool,
    /// 赋值符号两侧是否加空格。
    #[serde(default = "default_true")]
    pub around_assignment: bool,
    /// 冒号前是否加空格。
    #[serde(default = "default_true")]
    pub before_colon: bool,
    /// 冒号后是否加空格。
    #[serde(default = "default_true")]
    pub after_colon: bool,
    /// 一元运算符后是否加空格。
    #[serde(default = "default_false")]
    pub after_unary_operators: bool,
    /// `@` 之后、左括号之前是否加空格。
    #[serde(default = "default_false")]
    pub after_at: bool,
}

impl Default for SpaceConfig {
    fn default() -> Self {
        SpaceConfig {
            around_binary_operator: true,
            after_comma: true,
            after_semicolon: true,
            before_parens_in_function_call: false,
            before_control_statement_parens: false,
            inside_parens: false,
            around_assignment: true,
            before_colon: true,
            after_colon: true,
            after_unary_operators: false,
            after_at: false,
        }
    }
}

/// 模块相关配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    /// 模块参数列表的左括号 `(` 是否另起一行。
    #[serde(default = "default_true")]
    pub parameter_list_break_before_open_paren: bool,
    /// 模块端口列表的左括号 `(` 是否另起一行。
    #[serde(default = "default_true")]
    pub port_list_break_before_open_paren: bool,
    /// 实例端口列表的左括号 `(` 是否另起一行。
    #[serde(default = "default_true")]
    pub instance_port_list_break_before_open_paren: bool,
    /// 参数声明中的类型、名称、默认值是否按列对齐。
    #[serde(default = "default_true")]
    pub align_parameters: bool,
    /// 每个端口是否单独占一行。
    #[serde(default = "default_true")]
    pub newline_per_port: bool,
    /// 端口列表中方向关键字与端口名称是否对齐。
    #[serde(default = "default_true")]
    pub port_alignment: bool,
    /// 实例化时每个端口连接是否单独占一行。
    #[serde(default = "default_true")]
    pub newline_per_instance_port: bool,
}

impl Default for ModuleConfig {
    fn default() -> Self {
        ModuleConfig {
            parameter_list_break_before_open_paren: true,
            port_list_break_before_open_paren: true,
            instance_port_list_break_before_open_paren: true,
            align_parameters: true,
            newline_per_port: true,
            port_alignment: true,
            newline_per_instance_port: true,
        }
    }
}

/// Formatter 全局配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatterConfig {
    /// 每一级代码块的缩进空格数。
    #[serde(default = "default_indent_width")]
    pub indent_width: u32,
    /// 模块内部的第一级代码是否缩进一个 `indent_width`。
    #[serde(default = "default_false")]
    pub indent_module_contents: bool,
    /// 是否使用 Tab 进行缩进。
    #[serde(default = "default_false")]
    pub use_tab: bool,
    /// 一个 Tab 显示的宽度。
    #[serde(default = "default_indent_width")]
    pub tab_width: u32,

    /// 每行最大列数，0 表示不限制。
    #[serde(default = "default_column_limit")]
    pub column_limit: u32,
    /// 是否删除行尾多余空格与制表符。
    #[serde(default = "default_true")]
    pub trim_trailing_whitespace: bool,

    /// 空格配置。
    #[serde(default)]
    pub space: SpaceConfig,

    /// 连续空行的最大数量。
    #[serde(default = "default_one")]
    pub max_consecutive_blank_lines: u32,
    /// 两个 always/initial/function/task 块之间是否至少保留一个空行。
    #[serde(default = "default_true")]
    pub blank_line_between_procedures: bool,

    /// 行尾注释是否按列对齐。
    #[serde(default = "default_true")]
    pub align_trailing_comments: bool,
    /// 注释与代码之间的最少空格数。
    #[serde(default = "default_two")]
    pub comment_indent: u32,
    /// 行尾注释对齐到的列号。
    #[serde(default = "default_comment_column")]
    pub comment_column: u32,

    /// 同一连续块内的 `=`/`<=` 是否对齐。
    #[serde(default = "default_true")]
    pub align_assignments: bool,
    /// 实例化的端口连接是否按左右括号对齐。
    #[serde(default = "default_true")]
    pub align_instance_ports: bool,
    /// 实例端口连接括号内侧保留空格数。
    #[serde(default = "default_two")]
    pub space_inside_instance_port_parens: u32,
    /// case 语句中冒号前的表达式是否对齐。
    #[serde(default = "default_true")]
    pub align_case_items: bool,

    /// 模块相关配置。
    #[serde(default)]
    pub module: ModuleConfig,

    /// 实例化端口超过多少个时强制换行（0 不强制）。
    #[serde(default = "default_one")]
    pub wrap_instance_ports: u32,

    /// `begin` 是否另起一行。
    #[serde(default = "default_true")]
    pub begin_end_on_newline: bool,
    /// `begin` 后是否紧跟第一条语句。
    #[serde(default = "default_false")]
    pub end_of_line_for_begin: bool,
    /// `end` 是否必须单独占一行。
    #[serde(default = "default_false")]
    pub end_on_newline: bool,
    /// `else` 是否另起一行。
    #[serde(default = "default_true")]
    pub else_on_newline: bool,
    /// 延时控制语句是否可与相邻语句放在同一行。
    #[serde(default = "default_true")]
    pub delay_on_same_line: bool,

    /// 预处理器指令（`ifdef/`else/`endif/`define 等）是否从行首开始（不缩进）。
    #[serde(default = "default_true")]
    pub directives_at_line_start: bool,

    /// 是否统一 case/casez/casex 风格。
    #[serde(default)]
    pub reformat_case: ReformatCase,
    /// case 分支内容相对 case 的缩进层级。
    #[serde(default = "default_one")]
    pub case_indent_level: u32,
}

fn default_indent_width() -> u32 {
    4
}
fn default_comment_column() -> u32 {
    40
}
fn default_two() -> u32 {
    2
}
fn default_one() -> u32 {
    1
}
fn default_column_limit() -> u32 {
    100
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

impl Default for FormatterConfig {
    fn default() -> Self {
        FormatterConfig {
            indent_width: 4,
            indent_module_contents: false,
            use_tab: false,
            tab_width: 4,
            column_limit: 100,
            trim_trailing_whitespace: true,
            space: SpaceConfig::default(),
            max_consecutive_blank_lines: 1,
            blank_line_between_procedures: true,
            align_trailing_comments: true,
            comment_indent: 2,
            comment_column: 40,
            align_assignments: true,
            align_instance_ports: true,
            space_inside_instance_port_parens: 2,
            align_case_items: true,
            module: ModuleConfig::default(),
            wrap_instance_ports: 1,
            begin_end_on_newline: true,
            end_of_line_for_begin: false,
            end_on_newline: false,
            else_on_newline: true,
            delay_on_same_line: true,
            directives_at_line_start: true,
            reformat_case: ReformatCase::None,
            case_indent_level: 1,
        }
    }
}

/// 加载 TOML 格式的配置文件。
pub fn load_from_path(path: &Path) -> Result<FormatterConfig, ConfigError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::Read(path.display().to_string(), e))?;
    toml::from_str(&text).map_err(|e| ConfigError::Parse("TOML".into(), e.to_string()))
}

/// 导出默认配置为 TOML 字符串。
pub fn dump_default_toml() -> String {
    toml::to_string(&FormatterConfig::default()).expect("TOML 序列化不应失败")
}

/// 配置层错误。
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("无法读取配置文件 {0}: {1}")]
    Read(String, std::io::Error),
    #[error("{0} 配置解析失败: {1}")]
    Parse(String, String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_docs() {
        let cfg = FormatterConfig::default();
        assert_eq!(cfg.indent_width, 4);
        assert!(!cfg.indent_module_contents);
        assert_eq!(cfg.column_limit, 100);
        assert!(cfg.trim_trailing_whitespace);
        assert!(cfg.space.around_binary_operator);
        assert!(cfg.space.after_comma);
        assert!(!cfg.space.before_control_statement_parens);
        assert!(cfg.module.port_alignment);
        assert_eq!(cfg.comment_column, 40);
        assert_eq!(cfg.space_inside_instance_port_parens, 2);
        assert!(cfg.directives_at_line_start, "预处理器指令应默认顶格");
    }

    #[test]
    fn dump_and_reload_roundtrip() {
        let toml_str = dump_default_toml();
        let cfg: FormatterConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(cfg.indent_width, 4);
        assert!(cfg.space.around_binary_operator);
        assert!(!cfg.space.before_control_statement_parens);
    }

    #[test]
    fn toml_partial_config_uses_defaults() {
        let text = "indent_width = 2\n[space]\nafter_at = true\n";
        let cfg: FormatterConfig = toml::from_str(text).unwrap();
        assert_eq!(cfg.indent_width, 2);
        assert!(cfg.space.after_at);
        assert!(cfg.space.around_binary_operator, "未指定项应使用默认值");
        assert_eq!(cfg.comment_column, 40);
    }

    #[test]
    fn toml_config_loads() {
        let text = "indent_width = 2\n[space]\nafter_at = true\n";
        let cfg: FormatterConfig = toml::from_str(text).unwrap();
        assert_eq!(cfg.indent_width, 2);
        assert!(cfg.space.after_at);
    }
}
