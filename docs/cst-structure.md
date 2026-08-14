# CST 结构说明

本文档记录 tree-sitter-systemverilog（v0.4.0）对 SystemVerilog 关键结构的
**实际 CST 结构**。所有结构均通过真实解析验证，未作任何猜测。

生成方式：

```bash
svfmt cst examples/param_counter.sv
# 或等价简写
svfmt examples/param_counter.sv
```

完整输出示例见 [cst_param_counter.txt](cst_param_counter.txt)。

## 1. 节点打印格式

每一行表示一个 CST 节点：

```
(kind named/unnamed) [起始行:起始列 - 结束行:结束列] "原始文本"
```

- `named` / `unnamed`：是否为命名节点（语法规则产生的节点为 named，token 如 `;` `,` `(` `module` 为 unnamed）
- `!ERROR`：该节点本身是语法错误节点
- `!MISSING`：缺少必要 token 时产生的占位节点
- `!has_error`：子树内包含错误

## 2. module 的 CST 结构

```systemverilog
module alu ( input wire clk, output logic [7:0] d ); endmodule
```

```
module_declaration
└── module_ansi_header
    ├── module_keyword                      // named: "module"
    ├── simple_identifier                   // named: 模块名（field: name）
    ├── parameter_port_list                 // 可选，见 §3
    └── list_of_port_declarations           // 见 §4
└── endmodule                               // unnamed token
```

**重要：空端口列表 `()` 时头部是 `module_nonansi_header` 而非 `module_ansi_header`**

```systemverilog
module top (); endmodule
```

```
module_declaration
└── module_nonansi_header
    ├── module_keyword
    ├── simple_identifier
    ├── parameter_port_list                 // 可选
    └── list_of_ports                       // 仅端口名列表
└── endmodule
```

区分规则：

| 端口写法 | header 节点 |
|---|---|
| `module top ();` | `module_nonansi_header` + `list_of_ports` |
| `module top ( input a, ... );` | `module_ansi_header` + `list_of_port_declarations` |
| 非 ANSI 风格（声明在 module 内部） | `module_nonansi_header` |

## 3. parameter list 的 CST 结构

```systemverilog
module core #(parameter DATA_WIDTH = 32, parameter logic [7:0] M = 8'hFF) ( input a );
```

```
module_ansi_header
└── parameter_port_list                     // "#(...)" 整体
    ├── #                                    // unnamed
    ├── (                                    // unnamed
    ├── parameter_port_declaration
    │   └── parameter_declaration
    │       ├── parameter                   // unnamed token
    │       ├── data_type_or_implicit       // 可选：有类型声明时存在（如 "logic [7:0]"）
    │       └── list_of_param_assignments
    │           └── param_assignment
    │               ├── simple_identifier   // 参数名
    │               ├── =                   // unnamed
    │               └── constant_param_expression
    ├── ,                                    // unnamed
    ├── parameter_port_declaration
    └── )                                    // unnamed
```

`param_assignment` 中 `simple_identifier`、`=`、`constant_param_expression`
是直接子节点（`constant_param_expression` 内部是完整的常量表达式节点树）。

## 4. port list 的 CST 结构

```systemverilog
module top ( input wire clk, output logic [7:0] data );
```

```
module_ansi_header
└── list_of_port_declarations
    ├── (                                    // unnamed
    ├── ansi_port_declaration                // field: port_name
    │   ├── net_port_header                 // net 类型端口
    │   │   ├── port_direction              // input/output/inout
    │   │   └── net_port_type               // wire / tri / wand ...
    │   │       └── net_type
    │   └── simple_identifier               // 端口名
    ├── ,
    ├── ansi_port_declaration
    │   ├── variable_port_header            // logic/reg 等变量类型端口
    │   │   ├── port_direction
    │   │   └── variable_port_type          // data_type -> integer_vector_type -> logic
    │   │       └── data_type + packed_dimension
    │   └── simple_identifier
    └── )
```

- `ansi_port_declaration` 有 field `port_name`，可直接取到端口名。
- net 端口头是 `net_port_header`，变量端口头是 `variable_port_header`。
- 宽度声明 `[7:0]` 是 `packed_dimension`（含 unnamed `[` `]` 与 named `constant_range`）。

## 5. ERROR node 处理

tree-sitter 对非法源码会生成 `ERROR` 节点（`is_error() == true`），
必要时生成 `MISSING` 节点（缺少 token）。打印时用 `!ERROR` / `!MISSING` 标记。

```bash
svfmt cst --fail-on-error broken.sv   # 打印 CST 且语法错误时退出码 1
```

第一阶段发现的一个真实案例：**模块实例化参数列表的尾随逗号**。

```systemverilog
scheduler #(
    .THREADS_PER_BLOCK(THREADS_PER_BLOCK),   // ← 尾随逗号
) scheduler_instance ( ... );
```

tree-sitter-systemverilog 按 IEEE 1800 严格语法将其解析为 `ERROR` 节点
（`list_of_parameter_assignments` 不允许尾随逗号），而 VCS/Questa 等 EDA
工具普遍宽容接受。见 `examples/core.sv`（已知 2 个 ERROR 节点）。

这验证了"支持 ERROR node"的设计必要性：Formatter 遇到 ERROR 必须
容忍并尽量原样输出，不得崩溃、不得修改语义。

## 6. 后续 Formatter 注意

- 空白与换行不产生节点，节点之间通过 `byte_range()` 还原原始文本。
- 注释是独立 named 节点（`one_line_comment` / `block_comment`），位置信息完整。
- preprocessor 指令是独立节点（`timescale_compiler_directive`、
  `default_nettype_compiler_directive` 等），其 `#`/`` ` `` 之后的文本为 named 子节点。
- 设计文档要求 `format(format(source)) == format(source)`（幂等性），
  后续所有规则都要用 golden test 验证。
