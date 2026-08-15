
# SystemVerilog Formatter 项目开发规范

我要开发一个专业的 SystemVerilog Formatter，项目目标是：

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

## 一、核心架构

必须采用以下架构：

Source Code
    ↓
tree-sitter-systemverilog
    ↓
Tree-sitter CST
    ↓
Formatter
    ↓
Formatted Source Code

不要自己重新实现 SystemVerilog Lexer。
不要自己重新实现 SystemVerilog Parser。
SystemVerilog 的语法解析必须由 tree-sitter-systemverilog 完成。

Formatter 的职责是：
1. 遍历 Tree-sitter CST
2. 识别 syntax node 和 token
3. 根据 Formatter Configuration 决定：
   - 空格
   - 缩进
   - 换行
   - 括号位置
   - 对齐
   - 注释布局
4. 输出格式化后的 SystemVerilog source code

Formatter 不负责：
- 修改 HDL 语义
- 修改表达式含义
- 自动把 always 转换成 always_ff
- 自动把 wire 转换成 logic
- 自动修改变量类型
- 自动修改 port 顺序
- 自动修改 parameter 顺序

Formatter 必须是 source-to-source formatting tool。

---

# 二、技术栈

优先使用 Rust。

建议：

Rust
├── tree-sitter
├── tree-sitter-systemverilog
├── serde
├── serde_yaml
└── clap

配置格式优先使用 YAML。

例如：

svfmt
├── src/
│   ├── parser/
│   ├── formatter/
│   ├── config/
│   ├── output/
│   └── cli/
├── tests/
├── examples/
└── docs/

---

# 三、Parser 层

Parser 层只负责：

SystemVerilog source
        ↓
tree-sitter-systemverilog
        ↓
Tree-sitter Tree

Parser 层需要提供：

parse(source) -> Tree

并提供：

- root node
- node kind
- child nodes
- named children
- token text
- source range
- byte range
- line/column
- error node

不要在 Parser 层实现 Formatter 规则。

---

# 四、CST 使用原则

Formatter 必须以 CST 为主要输入。

不要把 CST 转换成一个会丢失源码信息的纯 AST，然后再从 AST 重新生成代码。

必须尽量保留：

- syntax node
- token
- source range
- comments
- whitespace information
- newline information
- preprocessor directives
- parentheses
- commas
- semicolons
- operators
- original token text

Formatter 的目标是：

尽可能做到：

format(format(source)) == format(source)

也就是 Formatter 必须具有幂等性。

---

# 五、Formatter 架构

Formatter 建议拆分成以下模块：

Formatter
├── traversal
├── indentation
├── whitespace
├── line_break
├── alignment
├── comments
├── module
├── declarations
├── expressions
├── statements
├── ports
├── parameters
└── preprocessor

不要把所有格式化逻辑写到一个 formatter.rs 中。

---

# 六、Formatter 工作方式

不要简单地：

CST → 修改字符串 → 输出

建议建立一个 Formatting Token / Document 层。

例如：

CST
 ↓
Formatting Commands
 ↓
Document
 ↓
Line Breaking
 ↓
Output

Formatting Document 可以包含：

Text
Space
Newline
Indent
Dedent
SoftLine
HardLine
Group
Align

例如：

module core #( ... ) (...);

内部可以表示成：

Text("module")
Space
Text("core")
Space
Text("#")
SoftLine
Text("(")
...
Text(")")
Space
Text("(")
...
Text(")")
Text(";")

最后由 line-breaking algorithm 根据 ColumnLimit 决定 SoftLine 是否变成 Space 或 Newline。

---

# 七、必须支持的基础格式化功能

第一阶段至少支持：

1. Indentation
2. Spaces
3. Blank lines
4. begin/end
5. if/else
6. case
7. module
8. parameter list
9. port list
10. declarations
11. assignments
12. expressions
13. comments
14. preprocessor directives
15. long-line breaking

---

# 八、配置系统

所有 Formatter 行为必须通过 Configuration 控制，参考 @verilog_format.md 。



# 二十、CLI
```bash
svfmt file.sv
svfmt --config config.toml file.sv
svfmt -o file_after_format.sv file.sv

svfmt --in-place file.sv
svfmt --version
svfmt -o file_afte_format.sv file.sv

# 将选项默认值生成配置文件
svfmt --dump-config
```

# 二十一、重要设计原则

1. 不重新实现 SystemVerilog parser。
2. 使用 tree-sitter-systemverilog。
3. CST 是 Formatter 的主要输入。
4. 不依赖完整 AST 才能格式化。
5. 不改变 HDL 语义。
6. 不修改 comment 内容。
7. 不破坏 preprocessor。
8. 支持 ERROR node。
9. Formatter 必须幂等。
10. 所有格式行为必须可配置。
11. 所有配置必须有测试。
12. 不要使用正则表达式实现 SystemVerilog parser。
13. Formatter 逻辑必须模块化。
14. 不要为了实现某个格式规则而修改 CST。
15. Parser、CST、Formatter、Configuration、CLI 必须解耦。

---

# 二十二、开发要求

每次实现一个功能时：

1. 先检查现有代码。
2. 找到对应 CST node。
3. 确认 Tree-sitter 实际产生的 node structure。
4. 再设计 Formatter rule。
5. 添加 configuration。
6. 添加 unit test。
7. 添加 golden test。
8. 运行相关测试。
9. 运行完整测试。
10. 检查 formatter 的 idempotency。

不要猜测 tree-sitter-systemverilog 的 node 名称。

如果不确定某个 node 的结构，先通过实际 parser 输出 CST/tree 来确认。

不要假设 SystemVerilog grammar 的结构。

---

# 测试
```bash
./target/release/svfmt examples/alu.sv -o examples/alu_tmp.sv && diff examples/alu_tmp.sv examples/alu_expected.sv 

./target/release/svfmt examples/controller.sv -o examples/controller_tmp.sv && diff -b examples/controller_tmp.sv examples/controller_expected.sv 

./target/release/svfmt examples/core.sv -o examples/core_tmp.sv && diff examples/core_tmp.sv examples/core_expected.sv 
./target/release/svfmt examples/hdmi.sv -o examples/hdmi_tmp.sv && diff examples/hdmi_tmp.sv examples/hdmi_expected.sv 
./target/release/svfmt examples/taxi.sv -o examples/taxi_tmp.sv && diff examples/taxi_tmp.sv examples/taxi_expected.sv 

./target/release/svfmt examples/test1.sv -o examples/test1_tmp.sv && diff examples/test1_tmp.sv examples/test1_expected.sv 

./target/release/svfmt examples/test2.sv -o examples/test2_tmp.sv && diff examples/test2_tmp.sv examples/test2_expected.sv 
./target/release/svfmt examples/test3.sv -o examples/test3_tmp.sv && diff examples/test3_tmp.sv examples/test3_expected.sv 
```



# 二十三、第一阶段任务

现在不要实现完整 Formatter。

第一步只实现：

1. 创建 Rust 项目。
2. 集成 tree-sitter。
3. 集成 tree-sitter-systemverilog。
4. 读取一个 .sv 文件。
5. Parser 生成 CST。
6. 递归打印 CST。
7. 打印每个 node 的：
   - kind
   - named/unnamed
   - start position
   - end position
   - text
8. 正确处理 ERROR node。
9. 编写测试验证 module、parameter list、port list 可以被正确解析。


完成以上内容以后暂停，不要继续实现 Formatter。

输出：
1. 项目目录
2. Cargo.toml
3. 代码
4. 测试
5. 一个真实 SystemVerilog 示例
6. CST 输出示例
7. 说明 tree-sitter-systemverilog 对 module、parameter list、port list 的实际 CST 结构。

后续开发必须建立在实际 CST 结构之上，不允许猜测。


