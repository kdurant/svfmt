# svfmt — SystemVerilog 格式化工具

`svfmt` 是一个基于 [tree-sitter-systemverilog](https://github.com/tree-sitter/tree-sitter-systemverilog) 的 SystemVerilog 源码格式化工具。它先把源码解析成 CST，再根据可配置的规则重新排版输出，**只调整排版、不改动任何 HDL 语义**。

```
SystemVerilog Source
      │
      ▼
tree-sitter-systemverilog
      │
      ▼
        CST
      │
      ▼
   Formatter
      │
      ▼
Formatted SystemVerilog
```

## 特性

- **语法级解析**：基于 tree-sitter，不做字符串/正则替换，不会破坏代码结构
- **纯 source-to-source**：只改空格、缩进、换行、括号位置、对齐与注释布局；不修改语义、不改变表达式含义、不改动端口/参数顺序
- **高度可配置**：缩进、空格、空行、注释、对齐、模块/端口/实例布局等 40+ 项选项
- **配置文件**：支持 TOML
- **管道友好**：支持 stdin/stdout、`-o` 输出文件、`--in-place` 就地覆盖
- **调试支持**：内置 CST 打印（`--cst` / `cst` 子命令）

## 安装

需要 Rust 工具链（稳定版）。

```bash
git clone <repo-url> svfmt
cd svfmt
cargo build --release
```

构建产物位于 `target/release/svfmt`，可将其加入 `PATH`：

```bash
cp target/release/svfmt ~/.local/bin/
```

## 快速开始

### 基本用法

```bash
# 格式化文件，结果输出到 stdout
svfmt top.sv

# 输出到指定文件
svfmt top.sv -o top_formatted.sv

# 就地覆盖源文件
svfmt top.sv --in-place

# 从 stdin 读取，结果写到 stdout（省略 FILE 或传 `-`）
cat top.sv | svfmt - > formatted.sv
```

## 命令行选项

```
Usage: svfmt [OPTIONS] [FILE] [COMMAND]

Commands:
  cst   解析 SystemVerilog 文件并递归打印 CST

Arguments:
  [FILE]  输入 .sv 文件路径；省略或为 `-` 时从标准输入读取

Options:
  -o, --output <OUT>     输出文件路径（默认输出到 stdout）
      --in-place         就地格式化（覆盖输入文件）
      --config <CONFIG>  配置文件路径（TOML）
      --dump-config      打印默认配置并退出
      --cst              解析并打印 CST（调试用）
  -h, --help             Print help
  -V, --version          Print version
```

### `cst` 子命令（调试）

```bash
svfmt cst top.sv                    # 打印 CST 树
svfmt cst top.sv --indent-width 4   # 调整打印缩进
svfmt cst top.sv --max-text-len 40  # 截断超长节点文本
svfmt cst top.sv --fail-on-error    # 存在语法错误时以非零状态退出
```

## 配置

格式化行为由 `FormatterConfig` 控制，每个选项都有默认值。完整选项说明见 [`verilog_format.md`](verilog_format.md)。

### 导出默认配置

```bash
svfmt --dump-config
```

输出一份 TOML 格式的默认配置，可直接保存为配置文件后修改。

### 使用配置文件

```bash
svfmt top.sv --config svfmt.toml
```

配置文件使用 TOML 格式（`.toml` 扩展名）。未填写的选项使用默认值。

示例 `svfmt.toml`：

```toml
indent_width = 4
use_tab = false
column_limit = 100
align_trailing_comments = true
comment_column = 40

[space]
around_binary_operator = true
after_comma = true
before_control_statement_parens = true

[module]
newline_per_port = true
port_alignment = true

begin_end_on_newline = true
```

### 常用配置示例

**2 空格缩进 + `if (...)` 风格（控制语句前加空格）**

```toml
indent_width = 2

[space]
before_control_statement_parens = true
```

**不限制行宽 + Tab 缩进**

```toml
column_limit = 0
use_tab = true
tab_width = 4
```

## 注意事项

- 工具只负责排版，**不修改代码语义**：不会自动把 `always` 改成 `always_ff`、把 `wire` 改成 `logic`，也不会改动端口/参数顺序
- 对语法错误（ERROR 节点）的代码段会尽量原样保留；如需检查语法，可用 `svfmt cst --fail-on-error`
