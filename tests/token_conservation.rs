//! 语义守恒回归测试：格式化不得增删任何 token，且必须幂等。
//!
//! 这是"静默丢失"类缺陷的通用防线。golden（examples）只能覆盖已知样例，
//! 而此类缺陷的成因通常是"格式化函数对未预料的 CST 子节点静默忽略"，因此
//! 这里用一组结构各异的构造片段（含宏、限定符、位置连接、ERROR 恢复、
//! 未结构化节点等）断言：
//!   1. {输出叶子 token 多重集} == {源码叶子 token 多重集}
//!   2. format(format(x)) == format(x)
//!
//! **唯一例外**：连接列表中的"残余分隔符"ERROR 节点（源文件自身的语法错误
//! 残留，如 `#(.A(1),)` 的尾随逗号）不输出——原样保留会让格式化结果继续无法
//! 解析。该类节点由 `residual_separator_is_dropped_output_parses` 单独覆盖；
//! 其它 ERROR 恢复区内容一律原样保留，见 `error_content_is_preserved`。

use svfmt::config::FormatterConfig;
use svfmt::formatter::Formatter;
use svfmt::formatter::tokens::leaf_tokens;
use svfmt::parser::SvParser;

fn fmt(src: &str) -> String {
    Formatter::format_source(src, &FormatterConfig::default()).expect("格式化应成功")
}

/// 叶子 token 多重集（排序后）。
fn token_multiset(src: &str) -> Vec<String> {
    let mut parser = SvParser::new().expect("grammar 应可加载");
    let tree = parser.parse(src).expect("解析应成功");
    let mut toks: Vec<String> = leaf_tokens(tree.root_node())
        .iter()
        .map(|t| t.text.to_string())
        .collect();
    toks.sort();
    toks
}

fn check(name: &str, src: &str) {
    let out = fmt(src);
    assert_eq!(
        token_multiset(&out),
        token_multiset(src),
        "[{name}] 格式化前后 token 不一致（静默丢失或新增）\n--- 输出 ---\n{out}"
    );
    let twice = fmt(&out);
    assert_eq!(
        out, twice,
        "[{name}] 不幂等\n--- 一次 ---\n{out}\n--- 二次 ---\n{twice}"
    );
}

#[test]
fn tokens_are_conserved_and_idempotent() {
    let cases: &[(&str, &str)] = &[
        // ---- 实例化：位置连接 / 位置参数 / 多实例 / 通配 ----
        ("ordered_ports", "module t;\nfoo u(a, b);\nendmodule\n"),
        (
            "ordered_ports_concat",
            "module t;\nfoo u(a, {x, y});\nendmodule\n",
        ),
        (
            "ordered_params",
            "module t;\nfoo #(8, 4) u(.a(a));\nendmodule\n",
        ),
        (
            "multi_instance_empty",
            "module t;\nfoo a(), b();\nendmodule\n",
        ),
        (
            "multi_instance_ports",
            "module t;\nfoo a(.x(1)), b(.y(2));\nendmodule\n",
        ),
        ("wildcard", "module t;\nfoo u(\n.*\n);\nendmodule\n"),
        // ---- 注释布局：每种位置都不得丢失/重复，且必须幂等 ----
        // （注释是"静默丢失"的高发区：连接列表首位、括号内、值内部、参数段、
        //   声明内部、过程块行尾、独立行等）
        (
            "comment_leading_in_port_list",
            "module t;\nfoo u(\n    // lead\n    .a(1)\n);\nendmodule\n",
        ),
        (
            "comment_middle_in_port_list",
            "module t;\nfoo u(\n    .a(1),\n    // why\n    .b(2)\n);\nendmodule\n",
        ),
        (
            "comment_in_empty_parens",
            "module t;\nfoo u( /* keep */ );\nendmodule\n",
        ),
        (
            "comment_inside_conn_value",
            "module t;\nfoo u(.a(/* c */ 1));\nendmodule\n",
        ),
        (
            "comment_after_conn_inline",
            "module t;\nfoo u(.a(1) /* c */, .b(2));\nendmodule\n",
        ),
        (
            "comment_in_param_list",
            "module t;\nfoo #(/* c */ 8) u();\nendmodule\n",
        ),
        (
            "comment_after_param",
            "module t;\nfoo #(8 /* c */) u();\nendmodule\n",
        ),
        (
            "comment_inside_declaration",
            "module t;\nlogic [9:0] x [4] /* verilator public_flat */ ;\nendmodule\n",
        ),
        (
            "comment_trailing_on_decl",
            "module t;\nlogic a; // keep\nlogic bb; // keep2\nendmodule\n",
        ),
        (
            "comment_in_seq_block_assigns",
            "module t;\ninitial begin\n  a = 1; // x\n  bbb = 2; // y\nend\nendmodule\n",
        ),
        (
            "comment_in_case_item",
            "module t;\ninitial begin\n  case (s)\n    A: x = 1; // a\n    B: y = 2; // b\n  endcase\nend\nendmodule\n",
        ),
        (
            "comment_in_generate",
            "module t;\ngenvar i;\ngenerate\n  for (i = 0; i < 2; i++) begin : g\n    // why\n    assign a[i] = 1;\n  end\nendgenerate\nendmodule\n",
        ),
        (
            "named_ports_comment",
            "module t;\nfoo u(\n    .a(a),   // 注释\n    .bb(b)\n);\nendmodule\n",
        ),
        (
            "macro_port_between",
            "module t;\nfoo #(\n    .W(W),\n    `ifdef X\n    .D(D),\n    `endif\n    .C(C)\n) u();\nendmodule\n",
        ),
        // ---- 声明 / 赋值 ----
        (
            "multi_param",
            "module m #( parameter A = 1, B = 2 )\n( input logic clk );\nendmodule\n",
        ),
        (
            "macro_params",
            "module m #(`INV(a), `INV(b))\n( input logic clk );\nendmodule\n",
        ),
        (
            "type_param",
            "module m #(parameter type T = int);\nendmodule\n",
        ),
        (
            "assign_multi",
            "module t;\nassign a = 1, b = 2;\nendmodule\n",
        ),
        ("assign_delay", "module t;\nassign #1 a = b;\nendmodule\n"),
        (
            "assign_strength",
            "module t;\nassign (strong0, weak1) a = b;\nendmodule\n",
        ),
        (
            "assign_error",
            "module t;\ninitial begin\n  a = 1, b = 2;\n  c++;\nend\nendmodule\n",
        ),
        // ---- 循环 ----
        (
            "do_while",
            "module t;\ninitial begin\n  do i++; while (i < 4);\nend\nendmodule\n",
        ),
        (
            "do_while_block",
            "module t;\ninitial begin\n  do begin\n    i++;\n  end while (i < 4);\nend\nendmodule\n",
        ),
        (
            "foreach",
            "module t;\ninitial begin\n  foreach (arr[i]) arr[i] = i;\nend\nendmodule\n",
        ),
        (
            "forever_while_repeat",
            "module t;\ninitial begin\n  forever a++;\n  while (b) b--;\n  repeat (3) c++;\nend\nendmodule\n",
        ),
        (
            "for_empty",
            "module t;\ninitial begin\n  for (;;) begin\n    a = 1;\n  end\nend\nendmodule\n",
        ),
        // ---- case ----
        (
            "unique_case",
            "module t;\nalways_comb\n  unique case (a)\n    1: b = 1;\n    default: b = 0;\n  endcase\nendmodule\n",
        ),
        (
            "priority_casez",
            "module t;\nalways_comb\n  priority casez (a)\n    4'b1???: b = 1;\n    default: b = 0;\n  endcase\nendmodule\n",
        ),
        (
            "case_inside",
            "module t;\nalways_comb begin\n  case (a) inside\n    [0:3]: b = 1;\n    default: b = 0;\n  endcase\nend\nendmodule\n",
        ),
        (
            "unique0_if",
            "module t;\nalways_comb begin\n  unique0 if (a) b = 1;\n  else b = 0;\nend\nendmodule\n",
        ),
        (
            "generate_case_default",
            "module t;\ngenerate\n  case (W)\n    8: begin : g8\n      assign a = 1;\n    end\n    default: begin : gd\n      assign a = 0;\n    end\n  endcase\nendgenerate\nendmodule\n",
        ),
        (
            "case_comment_before_item",
            "module t;\nalways_comb begin\n  case (a)\n    // 注释\n    1: b = 1;\n  endcase\nend\nendmodule\n",
        ),
        // ---- 表达式 / 位选 ----
        (
            "call_bit_select",
            "module t;\ninitial begin\n  x = f(i[7:0]);\nend\nendmodule\n",
        ),
        (
            "if_bit_select",
            "module t;\nalways_comb begin\n  if (a[7:0] != b[3 : 0]) c = 1;\nend\nendmodule\n",
        ),
        (
            "reduce_unary",
            "module t;\nalways_comb begin\n  if (& frame_len == 0) c = 1;\nend\nendmodule\n",
        ),
        // ---- 过程块 / 并发块 ----
        (
            "fork_join",
            "module t;\ninitial begin\n  fork\n    a = 1;\n    b = 2;\n  join\nend\nendmodule\n",
        ),
        (
            "fork_join_any_label",
            "module t;\ninitial begin : b1\n  fork : f1\n    a = 1;\n  join_any\nend\nendmodule\n",
        ),
        (
            "nested_fork",
            "module t;\ninitial begin\n  fork\n    begin\n      fork\n        a = 1;\n      join\n    end\n  join\nend\nendmodule\n",
        ),
        // ---- task / function ----
        (
            "function_decl_comment",
            "module t;\nfunction int f();\n  a = 1;\n  // 注释\n  int y;\n  return a;\nendfunction\nendmodule\n",
        ),
        (
            "task_comment",
            "module t;\ntask t1();\n  a = 1;\n  // 注释\nendtask\nendmodule\n",
        ),
        (
            "empty_function",
            "module t;\nfunction void f();\nendfunction\nendmodule\n",
        ),
        // ---- 其它既有写法（防回归） ----
        (
            "typedef_enum",
            "module t;\ntypedef enum logic [1:0] {A, B, C} e_t;\ne_t x;\nendmodule\n",
        ),
        (
            "genvar_generate",
            "module t;\ngenvar i;\ngenerate\n  for (i = 0; i < 2; i++) begin : g\n    assign a[i] = 1;\n  end\nendgenerate\nendmodule\n",
        ),
        (
            "ifdef_in_module",
            "module t;\n`ifdef FOO\nlogic a;\n`else\nlogic b;\n`endif\nendmodule\n",
        ),
        (
            "randcase",
            "module t;\ninitial begin\n  randcase\n    1: a = 1;\n    2: a = 2;\n  endcase\nend\nendmodule\n",
        ),
        ("implicit_port", "module t;\nfoo u(.a, .b);\nendmodule\n"),
        (
            "port_default_value",
            "module t (input logic a = 1'b0, output logic b);\nendmodule\n",
        ),
    ];

    for (name, src) in cases {
        check(name, src);
    }
    assert!(cases.len() >= 40, "语料不应被裁剪");
}

/// 残余分隔符（源自身的语法错误残留）被丢弃，且输出不再有语法错误。
///
/// 这是"不得增删 token"的唯一例外：`#(.A(1),)` 的尾随逗号是 source 的语法
/// 错误，tree-sitter 恢复成 `ERROR` 节点；保留它会让格式化结果继续无法解析
/// （`examples/core.sv` 即此情况，其 golden 不含该逗号）。
#[test]
fn residual_separator_is_dropped_output_parses() {
    let cases = [
        "module t;\nfoo #(.A(1),) u();\nendmodule\n",
        "module t;\nfoo u(\n    .a(1),\n);\nendmodule\n",
    ];
    for src in cases {
        let out = fmt(src);
        assert!(!out.contains(",)"), "残余逗号未丢弃: {out}");
        let mut parser = SvParser::new().expect("grammar 应可加载");
        let tree = parser.parse(&out).expect("解析应成功");
        assert!(!tree.has_error(), "输出仍含语法错误: {out}");
        assert_eq!(out, fmt(&out), "应幂等: {out}");
    }
}

/// 非分隔符的 ERROR 恢复区内容必须原样保留（绝不丢内容）。
#[test]
fn error_content_is_preserved() {
    // `.B` 缺括号：tree-sitter 恢复为 ERROR，内容必须保留
    let src = "module t;\nfoo #(.A(1), .B) u();\nendmodule\n";
    let out = fmt(src);
    assert!(out.contains(".B"), "ERROR 恢复区内容被丢弃: {out}");
    assert_eq!(out, fmt(&out), "应幂等: {out}");
}
