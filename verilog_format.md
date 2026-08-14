# 缩进
## IndentWidth
每一级代码块的缩进空格数
- 默认值： 4

### IndentWidth: 2
```verilog
always_ff @(posedge clk) begin
  if (enable) begin
    data <= next_data;
  end
end
```

### IndentWidth: 4
```verilog
always_ff @(posedge clk) begin
    if (enable) begin
        data <= next_data;
    end
end
```

## IndentModuleContents
模块内部的第一级代码是否缩进一个 `IndentWidth`，`endmodule` 始终保持顶格

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

## ContinuationIndentWidth
一条语句因为过长而换行后，续行增加多少缩进。

- 默认： 4
  
### ContinuationIndentWidth: 2
```verilog
assign result = a + b + c + d +
  e + f + g;
```

### ContinuationIndentWidth: 4
```verilog
assign result = a + b + c + d +
    e + f + g;
```

## AlignContinuationLines
超长行换行后的续行是否对齐到赋值表达式（`=`、`<=` 等右侧第一个表达式）的起始列，而不是使用固定 `ContinuationIndentWidth`

- 默认： true

### false
```verilog
assign result = a + b + c + d +
    e + f + g;
```

### true
```verilog
assign axis_slave.tready = (state == ST_IDLE) ||
                           (state == ST_NEXT && !last) ||
                           (state == ST_WAIT);
```

## UseTab
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

## TabWidth
当 UseTab 为 true（或读取已含 Tab 的源文件）时，一个 Tab 显示的宽度

- 默认： 4

#### TabWidth: 2
```verilog
always_ff @(posedge clk) begin
  if (enable) begin
    data <= next_data;
  end
end
```

#### TabWidth: 8
```verilog
always_ff @(posedge clk) begin
        if (enable) begin
                data <= next_data;
        end
end
```

# 行

## ColumnLimit
每行最大列数（字符数）。超过后格式化器将尽量在运算符、逗号等位置换行。

- 默认： 0
- 可选值： 0（不限制）

### ColumnLimit: 40
```verilog
assign result = a + b + c +
    d + e + f;
```

### ColumnLimit: 0
```verilog
assign result = a + b + c + d + e + f;
```

## TrimTrailingWhitespace
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

## Space.AroundBinaryOperator
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

## Space.AfterComma
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

## Space.AfterSemicolon
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

## Space.BeforeParensInFunctionCall
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

## Space.BeforeControlStatementParens
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

## Space.InsideParens
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

## Space.AroundAssignment
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

## Space.BeforeColon
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

## Space.AfterColon
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

## Space.AfterUnaryOperators
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

## Space.AfterAt
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

## MaxConsecutiveBlankLines
连续空行的最大数量

- 默认： 1
- 可选值： 0（删除所有连续空行）

### MaxConsecutiveBlankLines: 0
```verilog
logic a;
logic b;
```

### MaxConsecutiveBlankLines: 2
```verilog
logic a;
logic b;


logic c;
```

## BlankLineBetweenProcedures
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

## AlignTrailingComments
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

## CommentIndent
注释与代码之间的最少空格数（当 AlignTrailingComments 为 false 时）

- 默认： 2

### CommentIndent: 1
```verilog
assign a = 1'b0; // a
```

### CommentIndent: 4
```verilog
assign a = 1'b0;    // a
```

## CommentColumn
当 AlignTrailingComments 为 true 时，行尾注释对齐到的列号

- 默认： 40

### CommentColumn: 20
```verilog
assign a = 1'b0;  // a
assign b = 1'b1;  // b
```

### CommentColumn: 40
```verilog
assign a = 1'b0;                      // a
assign b = 1'b1;                      // b
```

# 对齐

## AlignAssignments
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

## AlignInstancePorts
实例化的端口连接是否按左右括号对齐

- 默认： true

### false
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

## SpaceInsideInstancePortParens
当 `AlignInstancePorts` 为 true 时，实例端口连接左右括号内侧各保留多少个空格

- 默认： 2

### SpaceInsideInstancePortParens: 1
```verilog
u_foo u_foo (
    .clk            ( clk  ),
    .long_port_name ( data )
);
```

### SpaceInsideInstancePortParens: 2
```verilog
u_foo u_foo (
    .clk            (  clk   ),
    .long_port_name (  data  )
);
```

## AlignCaseItems
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

## Module.ParameterList.BreakBeforeOpenParen
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

## Module.PortList.BreakBeforeOpenParen
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

## Module.InstancePortList.BreakBeforeOpenParen
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

## Module.AlignParameters
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

## Module.NewlinePerPort
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

## Module.PortAlignment
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


## Module.NewlinePerInstancePort
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


## OneLineInterfaceInstantiation
接口实例化是否压缩为一行

- 默认： true

#### true
```verilog
if_axi_stream #(.DATA_WIDTH(8)) fifo_if();
```

#### false
```verilog
if_axi_stream #
(
    .DATA_WIDTH(8)
) fifo_if();
```

## InterfaceTypePrefix
识别接口实例化时使用的类型名前缀。格式化器会优先使用源码中 `interface` 声明收集到的接口名，仅当源码中未收集到接口名时才按前缀/后缀启发式匹配。

- 默认： `if_`

## InterfaceTypeSuffix
识别接口实例化时使用的类型名后缀。

- 默认： `_if`

两个配置项配合 `OneLineInterfaceInstantiation` 使用：类型名命中接口声明、或以指定前缀开头、或以指定后缀结尾时，视为接口并压缩为一行。若前缀/后缀置空字符串，则对应的启发式匹配会被禁用（仅靠 `interface` 声明识别）。

## WrapInstancePorts
实例化端口超过多少个时强制换行

- 默认： 1
- 可选值： 0（不强制换行）

#### WrapInstancePorts: 1
```verilog
u_foo u_foo (
    .clk(clk),
    .rst_n(rst_n)
);
```

#### WrapInstancePorts: 3
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

## BeginEndOnNewline
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

## EndOfLineForBegin
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

## EndOnNewline
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

## ElseOnNewline
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

## EndBlockLabels
是否在 `end` 后追加对应的块标签/模块名

- 默认： false

#### false
```verilog
end
```

#### true
```verilog
end : block_name
```

## 其他

## ReformatCase
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

## CaseIndentLevel
`case` 分支内容相对 `case` 的缩进层级

- 默认： 1

#### CaseIndentLevel: 0
```verilog
case (sel)
2'd0: data = 1'b0;
2'd1: data = 1'b1;
endcase
```

#### CaseIndentLevel: 1
```verilog
case (sel)
    2'd0: data = 1'b0;
    2'd1: data = 1'b1;
endcase
```
