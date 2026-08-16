//! CLI 层：命令入口与文件处理。

pub mod args;

use std::io::{self, Write};
use std::path::Path;

use clap::Parser;

use crate::cli::args::{Cli, Command, CstArgs};
use crate::config::{FormatterConfig, dump_default_toml, load_from_path};
use crate::formatter::Formatter;
use crate::output::{PrintOptions, print_cst_to_string};
use crate::parser::{SvParser, collect_error_nodes};

/// CLI 主入口，返回错误信息。
pub fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    if cli.dump_config {
        print!("{}", dump_default_toml());
        return Ok(());
    }

    match cli.command {
        Some(Command::Cst(args)) => run_cst(&args),
        None => run_format(&cli),
    }
}

/// 默认格式化流程。
fn run_format(cli: &Cli) -> Result<(), CliError> {
    // 支持文件路径或 stdin（无参数 / `-`）
    let source = match &cli.file {
        Some(f) if f.to_string_lossy() != "-" => read_source(f)?,
        _ => read_stdin()?,
    };

    if cli.cst {
        let options = PrintOptions::default();
        let mut parser = SvParser::new()?;
        let tree = parser.parse(&source)?;
        let rendered = print_cst_to_string(tree.root_node(), &options);
        let stdout = io::stdout();
        let mut out = stdout.lock();
        out.write_all(rendered.as_bytes())?;
        out.flush()?;
        return Ok(());
    }

    // 加载配置（默认值或配置文件）
    let cfg = if let Some(path) = &cli.config {
        load_from_path(path)?
    } else {
        FormatterConfig::default()
    };

    let (formatted, error_count) = Formatter::format_source_checked(&source, &cfg)?;
    if error_count > 0 {
        let msg = format!("警告: 解析到 {error_count} 个语法错误节点，输出可能不完整");
        if cli.fail_on_parse_error {
            return Err(CliError::SyntaxErrors(error_count));
        }
        eprintln!("{msg}");
    }

    // 输出
    if cli.in_place {
        let file = cli
            .file
            .as_ref()
            .filter(|f| f.to_string_lossy() != "-")
            .ok_or(CliError::MissingInput)?;
        std::fs::write(file, &formatted)
            .map_err(|e| CliError::Write(file.display().to_string(), e))?;
    } else if let Some(out) = &cli.output {
        std::fs::write(out, &formatted)
            .map_err(|e| CliError::Write(out.display().to_string(), e))?;
    } else {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        out.write_all(formatted.as_bytes())?;
        out.flush()?;
    }
    Ok(())
}

/// 从 stdin 读取全部内容。
fn read_stdin() -> Result<String, CliError> {
    let mut buf = String::new();
    io::Read::read_to_string(&mut io::stdin().lock(), &mut buf)
        .map_err(|e| CliError::ReadFile("<stdin>".into(), e))?;
    Ok(buf)
}

fn run_cst(args: &CstArgs) -> Result<(), CliError> {
    let source = read_source(&args.file)?;
    let mut parser = SvParser::new()?;
    let tree = parser.parse(&source)?;

    let options = PrintOptions {
        indent_width: args.indent_width,
        max_text_len: args.max_text_len,
    };
    let rendered = print_cst_to_string(tree.root_node(), &options);
    let stdout = io::stdout();
    let mut out = stdout.lock();
    out.write_all(rendered.as_bytes())?;
    out.flush()?;

    if tree.has_error() {
        let errors = collect_error_nodes(tree.root_node());
        let msg = format!(
            "警告: 解析到 {} 个语法错误节点（见上方 !ERROR 标记）",
            errors.len()
        );
        if args.fail_on_error {
            return Err(CliError::SyntaxErrors(errors.len()));
        }
        eprintln!("{msg}");
    }
    Ok(())
}

fn read_source(path: &Path) -> Result<String, CliError> {
    let bytes =
        std::fs::read(path).map_err(|e| CliError::ReadFile(path.display().to_string(), e))?;
    String::from_utf8(bytes).map_err(|_| CliError::NotUtf8(path.display().to_string()))
}

/// CLI 错误类型。
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("缺少输入文件：svfmt <file.sv>")]
    MissingInput,
    #[error("无法读取文件 {0}: {1}")]
    ReadFile(String, io::Error),
    #[error("无法写入文件 {0}: {1}")]
    Write(String, io::Error),
    #[error("文件不是合法的 UTF-8 文本: {0}")]
    NotUtf8(String),
    #[error("源码存在语法错误（{0} 个 ERROR 节点），已按要求以失败退出")]
    SyntaxErrors(usize),
    #[error(transparent)]
    Parse(#[from] crate::parser::SvParserError),
    #[error(transparent)]
    Format(#[from] crate::formatter::FormatterError),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error("写入输出失败: {0}")]
    Io(#[from] io::Error),
}
