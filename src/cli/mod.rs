//! CLI 层：命令入口与文件处理。

pub mod args;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

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
    // 展开输入文件（支持 glob）
    let files = expand_inputs(&cli.files)?;

    // 无输入 → 从 stdin 读取
    if files.is_empty() {
        return format_stdin(cli);
    }

    // `-o` 只允许配合单个输入文件
    if cli.output.is_some() && files.len() > 1 {
        return Err(CliError::OutputWithMultipleFiles);
    }

    // 加载配置（默认值或配置文件），所有文件共用同一配置
    let cfg = load_config(cli)?;

    // 逐文件处理，单个文件失败不影响其余文件
    let mut failures = Vec::new();
    for file in &files {
        if let Err(e) = format_file(cli, &cfg, file) {
            failures.push(format!("{}: {e}", file.display()));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(CliError::BatchFailed(failures))
    }
}

/// 展开输入参数：支持 glob 模式（`*`、`?`、`[...]`）。
fn expand_inputs(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, CliError> {
    let mut out = Vec::new();
    for input in inputs {
        let s = input.to_string_lossy();
        // `-` 表示 stdin，仅在无其他输入时生效（这里直接跳过）
        if s == "-" {
            continue;
        }
        if has_glob_meta(&s) {
            let paths = glob::glob(&s).map_err(|e| CliError::Glob(s.to_string(), e))?;
            let mut matched = Vec::new();
            for entry in paths {
                let p = entry.map_err(|e| CliError::GlobEntry(s.to_string(), e.into()))?;
                matched.push(p);
            }
            if matched.is_empty() {
                return Err(CliError::GlobNoMatch(s.to_string()));
            }
            matched.sort();
            out.extend(matched);
        } else {
            out.push(input.clone());
        }
    }
    Ok(out)
}

fn has_glob_meta(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

/// 格式化单个文件（或打印其 CST）。
fn format_file(cli: &Cli, cfg: &FormatterConfig, file: &Path) -> Result<(), CliError> {
    let source = read_source(file)?;

    if cli.cst {
        return print_cst(&source);
    }

    format_source(cli, cfg, &source, Some(file))
}

/// 从 stdin 读取并格式化（或打印 CST）。
fn format_stdin(cli: &Cli) -> Result<(), CliError> {
    let source = read_stdin()?;

    if cli.cst {
        return print_cst(&source);
    }

    let cfg = load_config(cli)?;

    format_source(cli, &cfg, &source, None)
}

/// 加载配置：默认值或配置文件，再叠加命令行覆盖项（优先级最高）。
fn load_config(cli: &Cli) -> Result<FormatterConfig, CliError> {
    let mut cfg = if let Some(path) = &cli.config {
        load_from_path(path)?
    } else {
        FormatterConfig::default()
    };
    cli.overrides.apply_to(&mut cfg);
    Ok(cfg)
}

/// 格式化一段源码并输出。
fn format_source(
    cli: &Cli,
    cfg: &FormatterConfig,
    source: &str,
    file: Option<&Path>,
) -> Result<(), CliError> {
    let (formatted, error_count) = Formatter::format_source_checked(source, cfg)?;
    if error_count > 0 {
        let where_ = file.map_or(String::new(), |f| format!("{}: ", f.display()));
        let msg = format!("警告: {where_}解析到 {error_count} 个语法错误节点，输出可能不完整");
        if cli.fail_on_parse_error {
            return Err(CliError::SyntaxErrors(error_count));
        }
        eprintln!("{msg}");
    }

    // 输出
    if cli.in_place {
        let f = file.ok_or(CliError::MissingInput)?;
        std::fs::write(f, &formatted).map_err(|e| CliError::Write(f.display().to_string(), e))?;
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

/// 打印源码的 CST（调试用）。
fn print_cst(source: &str) -> Result<(), CliError> {
    let options = PrintOptions::default();
    let mut parser = SvParser::new()?;
    let tree = parser.parse(source)?;
    let rendered = print_cst_to_string(tree.root_node(), &options);
    let stdout = io::stdout();
    let mut out = stdout.lock();
    out.write_all(rendered.as_bytes())?;
    out.flush()?;
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
    #[error("输出选项 -o 与多个输入文件冲突：-o 只能配合单个输入文件")]
    OutputWithMultipleFiles,
    #[error("glob 模式 {0} 无效: {1}")]
    Glob(String, glob::PatternError),
    #[error("glob 匹配 {0} 时出错: {1}")]
    GlobEntry(String, io::Error),
    #[error("glob 模式 {0} 没有匹配到任何文件")]
    GlobNoMatch(String),
    #[error("以下文件处理失败:\n{}", .0.join("\n"))]
    BatchFailed(Vec<String>),
    #[error(transparent)]
    Parse(#[from] crate::parser::SvParserError),
    #[error(transparent)]
    Format(#[from] crate::formatter::FormatterError),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    #[error("写入输出失败: {0}")]
    Io(#[from] io::Error),
}
