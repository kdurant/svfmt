//! CLI 层：解析命令行参数。

use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};

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
