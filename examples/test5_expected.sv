// 回归样例：以下每处都曾在格式化时静默丢失内容、改变语义或破坏幂等性。
// 覆盖 P0/P1 全部修复点：通配/位置连接、多实例、多赋值、多参数、
// 宏参数、inc/dec、do-while/foreach、case 限定符与 inside、generate default、
// assign 延时/驱动强度、parameter type、fork/join、task/function 体内声明与注释。

module m_param #
(
    parameter A = 1, B = 2
)
(
    input  logic clk
);

assign a = 1, b = 2;
assign #1 c               = d;
assign (strong0, weak1) e = f;

initial
begin
    i++;
    j--;
    k = i + 1;
end

endmodule

module m_macro #
(
    `INV(a),
    `INV(b)
)
(
    input  logic clk
);
endmodule

module m_type #
(
    parameter type T = int
);
endmodule

module m_inst;

foo u
(
    a,
    b
);

bar v
(
    .x (  1  ),
    .y (  2  )
);

baz p(), q();

qux r
(
    .x (  1  )
),
s
(
    .y (  2  )
);

zed #
(
    8,
    4
)
w
(
    .a (  a  )
);

corge z
(
    .*
);

endmodule

module m_loop;

initial
begin
    do
        i++;
    while(i < 4);
    do
    begin
        j++;
    end
    while(j < 4);
    foreach(arr[idx])
        arr[idx] = idx;
    forever
        k++;
    while(l)
        l--;
    repeat(3)
        m++;
end

endmodule

module m_case;

always_comb
    unique case(a)
        1       : b = 1;
        default : b = 0;
    endcase

always_comb
    priority casez(a)
        4'b1??? : b = 1;
        default : b = 0;
    endcase

always_comb
begin
    case(a) inside
        [0:3]   : b = 1;
        default : b = 0;
    endcase
end

always_comb
begin
    unique0 if(a)
        b = 1;
    else
        b = 0;
end

endmodule

module m_gen;

generate
    case(W)
        8:
        begin : g8
            assign a = 1;
        end
        default:
        begin : gd
            assign a = 0;
        end
    endcase
endgenerate

endmodule

module m_sub;

function automatic int f(input int x);
    int y;
    y = x + 1;
    // 函数体内的注释不能被丢弃
    return y;
endfunction

task automatic t1();
    a = 1;
    // 任务体内的注释不能被丢弃
    b = 2;
endtask

initial
begin
    fork
        a = 1;
        b = 2;
    join

    fork : f_lbl
        c = 1;
    join_any

    x = f(y[7:0]);
end

endmodule
