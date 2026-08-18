# 缩进
## indent_width
每一级代码块的缩进空格数
- 默认值： 4

### indent_width: 2
```verilog
always_ff @(posedge clk) begin
  if (enable) begin
    data <= next_data;
  end
end
```

### indent_width: 4
```verilog
always_ff @(posedge clk) begin
    if (enable) begin
        data <= next_data;
    end
end
```

## indent_module_contents
模块内部的第一级代码是否缩进一个 `indent_width`，`endmodule` 始终保持顶格

- 默认： false

#### false
```verilog
module manchester_encode
(
);

wire [63:0] next_slide;
assign next_slide = {slide_reg[62:0], rx_bit_data};

endmodule
```

#### true
```verilog
module manchester_encode
(
);

    wire [63:0] next_slide;
    assign next_slide = {slide_reg[62:0], rx_bit_data};

endmodule
```

## use_tab
是否使用 Tab 进行缩进

- 默认： false

#### false
```verilog
always_ff @(posedge clk) begin
    if (enable) begin
        data <= next_data;
    end
end
```

#### true
```verilog
always_ff @(posedge clk) begin
	if (enable) begin
		data <= next_data;
	end
end
```

## tab_width
当 use_tab 为 true（或读取已含 Tab 的源文件）时，一个 Tab 显示的宽度

- 默认： 4

#### tab_width: 2
```verilog
always_ff @(posedge clk) begin
  if (enable) begin
    data <= next_data;
  end
end
```

#### tab_width: 8
```verilog
always_ff @(posedge clk) begin
        if (enable) begin
                data <= next_data;
        end
end
```

# 行

## column_limit
每行最大列数（字符数）。超过后格式化器将在可断行点换行：
- **函数调用 / 系统任务调用的参数**（左括号后、逗号后）
- **结构化表达式**（二元运算符前、逗号后）
- **拼接 `{...}`**（逗号后，贪心填满每行）
- **实例端口连接**：对齐布局最宽行超限时回退为"换行模式"——保留名字对齐，
  值在逗号/运算符处断行（续行缩进 +1 级）

注意：**对齐段**（模块体连续赋值、seq_block 赋值段）依赖单行文本计算列宽，
为保持对齐结构稳定，其 RHS **不受 column_limit 影响**（始终单行）。
实例端口连接已改为超宽时换行（见上）。

- 默认： 100
- 可选值： 0（不限制）或正整数

### column_limit: 40
```verilog
initial
begin
    $display(
        "very long format string %d %d",
        a,
        b,
        c);
end
```

### column_limit: 0
```verilog
initial
begin
    $display("very long format string %d %d", a, b, c);
end
```

### 实例端口连接超宽时换行（保留名字对齐）
```verilog
ila_128x4096 ila_128x4096Ex01
(
    .clk    (  clk_125m  ),
    .probe0 (  {trg, laser_pulse, axis_preview_tvalid, axis_preview_tdata,
        axis_preview_tready, axis_preview_tlast, axis_preview_tlen,
        axis_gps_tdata, axis_gps_tvalid, axis_gps_tready, axis_gps_tlast,
        axis_process_if.tdata, axis_process_if.tvalid, axis_process_if.tready,
        axis_process_if.tlast}  )
);
```

### 对齐段保持单行（不受 column_limit 影响）
```verilog
assign y = a + b + c + d + e + f + g + h + i + j + k;
```

## trim_trailing_whitespace
是否删除行尾多余空格与制表符

- 默认： false

### false
```verilog
assign a = b;···
```

### true
```verilog
assign a = b;
```


# 空格

## space.around_binary_operator
二元运算符两侧是否加空格（`+`、`-`、`*`、`/`、`%`、`==`、`!=`、`&&`、`||`、`&`、`|`、`^` 等）

- 默认： true

### true
```verilog
assign c = a + b;
```

### false
```verilog
assign c = a+b;
```

## space.after_comma
逗号后是否加一个空格

- 默认： true

### true
```verilog
module m(input a, input b);
```

### false
```verilog
module m(input a,input b);
```

## space.after_semicolon
`for` 循环与实例端口连接中的分号后是否加空格

- 默认： true

### true
```verilog
for (int i = 0; i < N; i++) begin
```

### false
```verilog
for (int i = 0;i < N;i++) begin
```

## space.before_parens_in_function_call
函数/任务调用时，函数名与左括号之间是否加空格

- 默认： false

### false
```verilog
data = my_func(a, b);
```

### true
```verilog
data = my_func (a, b);
```

## space.before_control_statement_parens
控制语句（`if`、`for`、`while`、`case`、`switch` 等）与左括号之间是否加空格

- 默认： false

### false
```verilog
if(a && b) begin
end
```

### true
```verilog
if (a && b) begin
end
```

## space.inside_parens
圆括号内侧是否加空格

- 默认： false

### false
```verilog
assign c = (a + b) * (c + d);
```

### true
```verilog
assign c = ( a + b ) * ( c + d );
```

## space.around_assignment
连续赋值、过程赋值等赋值符号两侧是否加空格（`=`、`<=`、`+=` 等）

- 默认： true

### true
```verilog
assign a = b;
cnt <= cnt + 1;
```

### false
```verilog
assign a=b;
cnt<=cnt+1;
```

## space.before_colon
三元运算符、case 分支中的冒号前是否加空格

- 默认： true

### true
```verilog
assign y = sel ? a : b;
2'd0 : data = 1'b0;
```

### false
```verilog
assign y = sel ? a: b;
2'd0: data = 1'b0;
```

## space.after_colon
冒号后是否加空格

- 默认： true

### true
```verilog
assign y = sel ? a : b;
2'd0: data = 1'b0;
```

### false
```verilog
assign y = sel ? a :b;
2'd0:data = 1'b0;
```

## space.after_unary_operators
一元运算符（`!`、`~`、`-`）后是否加空格

- 默认： false

### false
```verilog
assign y = ~a + !b;
```

### true
```verilog
assign y = ~ a + ! b;
```

## space.after_at
`@` 之后、左括号之前是否加空格

- 默认： false

### false
```verilog
always @(posedge clk) begin
end
```

### true
```verilog
always @ (posedge clk) begin
end
```

# 空行

## max_consecutive_blank_lines
连续空行的最大数量

- 默认： 1
- 可选值： 0（删除所有连续空行）

### max_consecutive_blank_lines: 0
```verilog
logic a;
logic b;
```

### max_consecutive_blank_lines: 2
```verilog
logic a;
logic b;


logic c;
```

## blank_line_between_procedures
两个 `always`/`initial`/`function`/`task` 块之间是否至少保留一个空行

- 默认： true

### true
```verilog
always @(posedge clk) begin
    a <= b;
end

always @(posedge clk) begin
    c <= d;
end
```

### false
```verilog
always @(posedge clk) begin
    a <= b;
end
always @(posedge clk) begin
    c <= d;
end
```



# 注释

## align_trailing_comments
行尾注释是否按列对齐

- 默认： true

### false
```verilog
assign a = 1'b0; // a
assign long_signal = 1'b1; // long
```

### true
```verilog
assign a = 1'b0;           // a
assign long_signal = 1'b1; // long
```

## comment_indent
注释与代码之间的最少空格数（当 align_trailing_comments 为 false 时）

- 默认： 2

### comment_indent: 1
```verilog
assign a = 1'b0; // a
```

### comment_indent: 4
```verilog
assign a = 1'b0;    // a
```

## comment_column
当 align_trailing_comments 为 true 时，行尾注释对齐到的列号

- 默认： 40

### comment_column: 20
```verilog
assign a = 1'b0;  // a
assign b = 1'b1;  // b
```

### comment_column: 40
```verilog
assign a = 1'b0;                      // a
assign b = 1'b1;                      // b
```

# 对齐

## align_assignments
同一连续块内的 `=`/`<=` 是否对齐

- 默认： true

### false
```verilog
logic a;
logic long_signal;
assign a = 1'b0;
assign long_signal = 1'b1;
```

### true
```verilog
logic a;
logic long_signal;
assign a           = 1'b0;
assign long_signal = 1'b1;
```

## align_instance_ports
实例化的端口连接是否按左右括号对齐

- 默认： true

### false
保持原样
```verilog
u_foo u_foo (
    .clk(clk),
    .long_port_name(data)
);
```

### true
```verilog
u_foo u_foo (
    .clk            (  clk   ),
    .long_port_name (  data  )
);
```

## space_inside_instance_port_parens
当 `align_instance_ports` 为 true 时，实例端口连接左右括号内侧各保留多少个空格

- 默认： 2

### space_inside_instance_port_parens: 1
```verilog
u_foo u_foo (
    .clk            ( clk  ),
    .long_port_name ( data )
);
```

### space_inside_instance_port_parens: 2
```verilog
u_foo u_foo (
    .clk            (  clk   ),
    .long_port_name (  data  )
);
```

## align_case_items
`case` 语句中冒号前的表达式是否对齐

- 默认： true

### false
```verilog
case (sel)
    2'd0: data = 1'b0;
    2'd10: data = 1'b1;
endcase
```

### true
```verilog
case (sel)
    2'd0 : data = 1'b0;
    2'd10: data = 1'b1;
endcase
```



# 模块

## module.parameter_list_break_before_open_paren
模块参数列表的左括号 `(` 是否另起一行

- 默认： true

### true
```verilog
module m #
(
    parameter int WIDTH = 8
) (...);
```

### false
```verilog
module m #(
    parameter int WIDTH = 8
) (...);
```

## module.port_list_break_before_open_paren
模块端口列表的左括号 `(` 是否另起一行

- 默认： true

### true
```verilog
module m #(
    parameter int WIDTH = 8
)
(
    input logic clk
);
```

### false
```verilog
module m #(
    parameter int WIDTH = 8
)(
    input logic clk
);
```

## module.instance_port_list_break_before_open_paren
实例端口列表的左括号 `(` 是否另起一行

- 默认： true

### true
```verilog
u_foo u_foo
(
    .clk            (  clk   ),
    .long_port_name (  data  )
);
```

### false
```verilog
u_foo u_foo (
    .clk            (  clk   ),
    .long_port_name (  data  )
);
```

## module.align_parameters
参数声明中的类型、名称、默认值和赋值符号是否按列对齐

- 默认： true

### false
```verilog
parameter int WIDTH = 8,
parameter logic ENABLE = 1'b1,
parameter bit USE_REG = 1'b0
```

### true
```verilog
parameter int   WIDTH   = 8,
parameter logic ENABLE  = 1'b1,
parameter bit   USE_REG = 1'b0
```

## module.newline_per_port
每个端口是否单独占一行（ANSI 风格下）

- 默认： true

### true
```verilog
module m (
    input logic a,
    input logic b
);
```

### false
```verilog
module m (
    input logic a, input logic b
);
```

## module.port_alignment
端口列表中 `input`/`output`/`inout` 关键字以及端口名称是否对齐（按最长关键字补齐空格）

- 默认： true

### true
```verilog
module m (
    input        a,
    output logic b,
    inout  wire  c
);
```

### false
```verilog
module m (
    input  a,
    output logic b,
    inout wire c
);
```


## module.newline_per_instance_port
实例化时每个端口连接是否单独占一行

- 默认： true

#### true
```verilog
u_foo u_foo (
    .clk(clk),
    .rst_n(rst_n)
);
```

#### false
```verilog
u_foo u_foo (
    .clk(clk), .rst_n(rst_n)
);
```

# 端口与实例

## wrap_instance_ports
实例化端口超过多少个时强制换行

- 默认： 1
- 可选值： 0（不强制换行）

#### wrap_instance_ports: 1
```verilog
u_foo u_foo (
    .clk(clk),
    .rst_n(rst_n)
);
```

#### wrap_instance_ports: 3
```verilog
u_foo u_foo (.clk(clk), .rst_n(rst_n));

u_bar u_bar (
    .a(a),
    .b(b),
    .c(c),
    .d(d)
);
```

## 括号

## begin_end_on_newline
`begin` 是否另起一行

- 默认： true

#### false
```verilog
always @(posedge clk) begin
    data <= next_data;
end
```

#### true
```verilog
always @(posedge clk)
begin
    data <= next_data;
end
```

## end_of_line_for_begin
`begin` 后是否紧跟第一条语句（而非换行）

- 默认： false

#### false
```verilog
always @(posedge clk) begin
    data <= next_data;
end
```

#### true
```verilog
always @(posedge clk) begin data <= next_data; end
```

## end_on_newline
`end` 是否必须单独占一行

- 默认： false

#### false
```verilog
always @(posedge clk) begin
    data <= next_data;
end else begin
    data <= default_data;
end
```

#### true
```verilog
always @(posedge clk) begin
    data <= next_data;
end
else begin
    data <= default_data;
end
```

## else_on_newline
`else` 是否另起一行

- 默认： true

#### false
```verilog
if (a) begin
    b = 1;
end else begin
    b = 0;
end
```

#### true
```verilog
if (a) begin
    b = 1;
end
else begin
    b = 0;
end
```

## delay_on_same_line
独立的延时控制语句（如 `#1us;`）是否可与相邻语句放在同一行。启用后，格式化工具可将独立延时语句与其相邻语句保持在同一行；关闭后延时语句单独占一行。

- 默认： true

### true
```verilog
initial
begin
    rst = 0; #1us;
    rst = 1; #1us;
end
```

### false
```verilog
initial
begin
    rst = 0;
    #1us;
    rst = 1;
    #1us;
end
```

## 其他

## reformat_case
是否统一 `case`/`casez`/`casex` 风格

- 默认： `none`
- 可选值： `none`、`casez`、`casex`

#### none
```verilog
casex (sel)
    2'b1x: data = 1'b1;
    default: data = 1'b0;
endcase
```

#### casez
```verilog
casez (sel)
    2'b1?: data = 1'b1;
    default: data = 1'b0;
endcase
```

## case_indent_level
`case` 分支内容相对 `case` 的缩进层级

- 默认： 1

#### case_indent_level: 0
```verilog
case (sel)
2'd0: data = 1'b0;
2'd1: data = 1'b1;
endcase
```

#### case_indent_level: 1
```verilog
case (sel)
    2'd0: data = 1'b0;
    2'd1: data = 1'b1;
endcase
```
