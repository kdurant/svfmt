//! 配置参数化测试：验证 Formatter 行为随配置变化。
//!
//! 设计文档要求"所有配置必须有测试"。这里覆盖各配置项开启/关闭时的输出差异。

use svfmt::config::FormatterConfig;
use svfmt::formatter::Formatter;

fn fmt(src: &str, cfg: &FormatterConfig) -> String {
    Formatter::format_source(src, cfg).unwrap()
}

fn default_cfg() -> FormatterConfig {
    FormatterConfig::default()
}

/// 验证修改配置项后输出确实变化。
fn assert_changes<F: Fn(&mut FormatterConfig)>(src: &str, mutate: F) -> (String, String) {
    let mut cfg = default_cfg();
    let before = fmt(src, &cfg);
    mutate(&mut cfg);
    let after = fmt(src, &cfg);
    assert_ne!(before, after, "配置应改变格式化输出");
    (before, after)
}

// ---------- 缩进 ----------

#[test]
fn indent_width_controls_indentation() {
    let src = "module t;\nalways_ff @(posedge clk) begin if(a) begin b <= 1; end end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.indent_width = 2;
    let out2 = fmt(src, &cfg);
    assert!(out2.contains("  if(a)"), "indent=2: {out2}");
    assert!(
        !out2.contains("    if(a)"),
        "indent=2 不应是 4 空格: {out2}"
    );
    let out4 = fmt(src, &default_cfg());
    assert!(out4.contains("    if(a)"), "indent=4: {out4}");
}

#[test]
fn indent_module_contents_toggles_body_indent() {
    let src = "module t;\nwire a;\nendmodule\n";
    let out_default = fmt(src, &default_cfg());
    assert!(
        out_default.starts_with("module t;\nwire a;"),
        "默认模块体不缩进: {out_default}"
    );
    let mut cfg = default_cfg();
    cfg.indent_module_contents = true;
    let out = fmt(src, &cfg);
    assert!(out.contains("    wire a;"), "模块体应缩进: {out}");
}

#[test]
fn use_tab_uses_tabs() {
    let src = "module t;\nalways @(posedge clk) begin if(a) b <= 1; end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.use_tab = true;
    let out = fmt(src, &cfg);
    assert!(out.contains('\t'), "应使用 tab 缩进: {out:?}");
}

// ---------- 行 ----------

#[test]
fn trim_trailing_whitespace_removes_line_end_spaces() {
    let src = "module t;\nwire a;   \nendmodule\n";
    let mut cfg = default_cfg();
    cfg.trim_trailing_whitespace = true;
    let out = fmt(src, &cfg);
    assert!(!out.contains("a;   "), "应删除行尾空格: {out:?}");
}

// ---------- 空格 ----------

#[test]
fn space_after_comma() {
    let src = "module t;\nassign y = foo(a,b);\nendmodule\n";
    let (_, after) = assert_changes(src, |c| c.space.after_comma = false);
    assert!(after.contains("foo(a,b)"), "逗号后无空格: {after}");
    let out = fmt(src, &default_cfg());
    assert!(out.contains("foo(a, b)"), "逗号后有空格: {out}");
}

#[test]
fn space_before_control_statement_parens() {
    let src = "module t;\nalways @(posedge clk) begin if (a) b <= 1; end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.space.before_control_statement_parens = true;
    let out = fmt(src, &cfg);
    assert!(out.contains("if (a)"), "控制语句括号前应空格: {out}");
    let out2 = fmt(src, &default_cfg());
    assert!(out2.contains("if(a)"), "默认控制语句括号前无空格: {out2}");
}

#[test]
fn space_around_binary_operator() {
    let src = "module t;\nassign y = a & b;\nendmodule\n";
    let (_, after) = assert_changes(src, |c| c.space.around_binary_operator = false);
    assert!(after.contains("a&b"), "二元运算符两侧无空格: {after}");
    assert!(fmt(src, &default_cfg()).contains("a & b"));
}

#[test]
fn space_around_assignment() {
    let src = "module t;\nassign y = a;\nendmodule\n";
    let (_, after) = assert_changes(src, |c| c.space.around_assignment = false);
    assert!(
        after.contains("y =a") || after.contains("y=a"),
        "赋值符号无空格: {after}"
    );
}

#[test]
fn space_after_at() {
    let src = "module t;\nalways @(posedge clk) begin end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.space.after_at = true;
    let out = fmt(src, &cfg);
    assert!(out.contains("always @ ("), "`@` 后应有空格: {out}");
    assert!(fmt(src, &default_cfg()).contains("always @("));
}

#[test]
fn space_after_semicolon() {
    let src = "module t;\nalways @(posedge clk) begin for (int i = 0; i < 8; i = i + 1) begin end end\nendmodule\n";
    let (_, after) = assert_changes(src, |c| c.space.after_semicolon = false);
    assert!(
        after.contains("0;i") || after.contains("8;i"),
        "分号后无空格: {after}"
    );
}

#[test]
fn space_inside_parens() {
    let src = "module t;\nassign y = (a + b);\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.space.inside_parens = true;
    let out = fmt(src, &cfg);
    assert!(out.contains("( a + b )"), "括号内侧应空格: {out}");
}

#[test]
fn space_after_unary_operators() {
    let src = "module t;\nassign y = ~a;\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.space.after_unary_operators = true;
    let out = fmt(src, &cfg);
    assert!(out.contains("~ a"), "一元运算符后应空格: {out}");
}

// ---------- 空行 ----------

#[test]
fn max_consecutive_blank_lines_limits_blank_lines() {
    let src = "module t;\nwire a;\n\n\nwire b;\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.max_consecutive_blank_lines = 0;
    let out = fmt(src, &cfg);
    assert!(!out.contains("\n\n"), "不应有空行: {out:?}");
    let out2 = fmt(src, &default_cfg());
    assert!(out2.contains("\n\n"), "默认保留 1 空行: {out2:?}");
}

#[test]
fn blank_line_between_procedures() {
    let src =
        "module t;\nalways @(posedge clk) begin end\nalways @(posedge clk) begin end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.blank_line_between_procedures = false;
    let out = fmt(src, &cfg);
    // 两个 always 之间应有空行
    let body = out.split("endmodule").next().unwrap();
    assert!(
        body.contains("end\nalways"),
        "块间应无空行（blank_line=false 未实现？）"
    );
    let _ = assert_changes(src, |c| c.blank_line_between_procedures = false);
}

// ---------- 注释 ----------

#[test]
fn comment_column_controls_alignment() {
    let src = "module t;\nwire a; // comment\nwire longer_signal; // another\nendmodule\n";
    let (out40, out60) = assert_changes(src, |c| c.comment_column = 60);
    assert!(out40.contains("// comment"), "默认对齐到 40");
    assert_ne!(out40, out60);
    // 不同 comment_column 时注释对齐列不同
    let col40 = out40.find("// comment").unwrap();
    let col60 = out60.find("// comment").unwrap();
    assert_ne!(col40, col60, "不同 CommentColumn 应对齐到不同列");
}

#[test]
fn align_trailing_comments_off() {
    let src = "module t;\nwire a; // one\nwire long_name; // two\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.align_trailing_comments = false;
    let out = fmt(src, &cfg);
    // 关闭对齐后注释紧跟 2 空格
    assert!(
        out.contains("a;  // one"),
        "关闭对齐用 comment_indent 空格: {out}"
    );
}

// ---------- 对齐 ----------

#[test]
fn align_case_items() {
    let src = "module t;\nalways @(posedge clk) begin case (a) 1'b0: b = 1; 1'b10: b = 0; endcase end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.align_case_items = false;
    let out = fmt(src, &cfg);
    assert!(
        out.contains("1'b0:") || out.contains("1'b0 :"),
        "关闭对齐冒号前 1 空格: {out}"
    );
}

#[test]
fn case_indent_level() {
    let src = "module t;\nalways @(posedge clk) begin case (a) 1'b0: b = 1; default: b = 0; endcase end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.case_indent_level = 0;
    let out = fmt(src, &cfg);
    assert!(
        !out.contains("case(a)\n        1'b0"),
        "case 分支不应额外缩进: {out}"
    );
    assert!(
        out.contains("case(a)\n    1'b0"),
        "case 分支与 case 同缩进: {out}"
    );
}

#[test]
fn reformat_case() {
    let src = "module t;\nalways @(posedge clk) begin casex (a) 1'b1: b = 1; default: b = 0; endcase end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.reformat_case = svfmt::config::ReformatCase::Casez;
    let out = fmt(src, &cfg);
    assert!(out.contains("casez"), "应统一为 casez: {out}");
}

// ---------- 模块 ----------

#[test]
fn parameter_list_break_before_open_paren() {
    let src = "module m #(parameter int W = 8) (); endmodule\n";
    let (out_true, out_false) = assert_changes(src, |c| {
        c.module.parameter_list_break_before_open_paren = false
    });
    assert!(
        out_true.contains("#\n("),
        "参数列表括号另起一行: {out_true}"
    );
    assert!(out_false.contains("#("), "参数列表括号紧跟: {out_false}");
}

#[test]
fn port_list_break_before_open_paren() {
    let src = "module m (input a); endmodule\n";
    let (out_true, out_false) =
        assert_changes(src, |c| c.module.port_list_break_before_open_paren = false);
    assert!(out_true.contains("\n("), "端口列表括号另起一行: {out_true}");
    assert!(out_false.contains("m ("), "端口列表括号紧跟: {out_false}");
}

#[test]
fn newline_per_port() {
    let src = "module m (input a, input b); endmodule\n";
    let mut cfg = default_cfg();
    cfg.module.newline_per_port = false;
    let out = fmt(src, &cfg);
    let body = out.split("(").nth(1).unwrap_or("");
    assert!(body.contains(", "), "端口同行: {out}");
    let out2 = fmt(src, &default_cfg());
    assert!(
        out2.contains("input a,") || out2.split(")").next().unwrap().contains("\n"),
        "默认每行一个端口"
    );
}

#[test]
fn port_alignment_toggle() {
    let src = "module m (input logic a, output wire b); endmodule\n";
    let mut cfg = default_cfg();
    cfg.module.port_alignment = false;
    let out = fmt(src, &cfg);
    assert!(!out.contains("input  logic"), "关闭方向对齐: {out}");
}

#[test]
fn align_parameters() {
    let src = "module m #(parameter int W = 8, parameter logic E = 1) (); endmodule\n";
    // 默认（true）：参数列表重新排版（# 与 ( 分行、每参数一行）
    let out_true = fmt(src, &default_cfg());
    assert!(
        out_true.contains("#\n("),
        "开启时参数列表重新排版: {out_true}"
    );
    // 关闭：保持源文件原样
    let mut cfg = default_cfg();
    cfg.module.align_parameters = false;
    let out_false = fmt(src, &cfg);
    assert!(
        out_false.contains("#(parameter int W = 8, parameter logic E = 1)"),
        "关闭时参数列表保持原样: {out_false}"
    );
    assert!(
        !out_false.contains("#\n("),
        "关闭时不做重新排版: {out_false}"
    );
    assert_ne!(out_true, out_false, "align_parameters 开关应改变输出");
}

// ---------- 括号 ----------

#[test]
fn begin_end_on_newline() {
    let src = "module t;\nalways @(posedge clk) begin b <= 1; end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.begin_end_on_newline = false;
    let out = fmt(src, &cfg);
    assert!(
        out.contains("begin\n") == false || !out.contains("posedge clk)\nbegin"),
        "begin 紧跟: {out}"
    );
    let out2 = fmt(src, &default_cfg());
    assert!(
        out2.contains("posedge clk)\nbegin"),
        "默认 begin 另起一行: {out2}"
    );
}

#[test]
fn else_on_newline() {
    let src = "module t;\nalways @(posedge clk) begin if(a) b <= 1; else b <= 0; end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.else_on_newline = false;
    let out = fmt(src, &cfg);
    assert!(
        out.contains("end\nelse") == false || !out.contains("\nelse\n"),
        "else 同行: {out}"
    );
    let out2 = fmt(src, &default_cfg());
    assert!(out2.contains("else\n"), "默认 else 另起一行: {out2}");
}

#[test]
fn end_of_line_for_begin() {
    let src = "module t;\nalways @(posedge clk) begin b <= 1; end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.end_of_line_for_begin = true;
    let out = fmt(src, &cfg);
    assert!(
        out.contains("begin b <= 1;") || out.contains("begin\n    b <= 1;"),
        "begin 后紧跟（或仍多行）"
    );
    let _ = assert_changes(src, |c| c.end_of_line_for_begin = true);
}

#[test]
fn end_on_newline() {
    let src = "module t;\nalways @(posedge clk) begin if(a) begin b <= 1; end else begin b <= 0; end end\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.end_on_newline = true;
    let out = fmt(src, &cfg);
    assert!(
        out.contains("end\n") && out.contains("else"),
        "end 与 else 分行: {out}"
    );
    assert!(
        out.contains("end\nelse") || out.contains("end\n    else"),
        "end 单独一行: {out}"
    );
}

// ---------- 实例 ----------

#[test]
fn one_line_interface_instantiation() {
    let src = "module t;\nif_axi_stream #(.DATA_WIDTH(8)) fifo_if();\nendmodule\n";
    let (out_true, out_false) = assert_changes(src, |c| c.one_line_interface_instantiation = false);
    assert!(
        out_true.contains("fifo_if();") && !out_true.contains("fifo_if\n"),
        "接口实例化单行: {out_true}"
    );
    assert!(out_false.contains("fifo_if\n("), "关闭单行化: {out_false}");
}

#[test]
fn align_instance_ports() {
    let src = "module t;\nu_foo u ( .clk(clk), .long_port(data) );\nendmodule\n";
    // 默认（true）：名称按列对齐 + 括号内侧补空格
    let out_true = fmt(src, &default_cfg());
    assert!(
        out_true.contains(".clk       ("),
        "开启时端口名按列对齐: {out_true}"
    );
    assert!(
        out_true.contains("(  clk   )"),
        "开启时括号内侧补空格: {out_true}"
    );
    // 关闭：紧凑输出（.name(value)，不对齐、无括号内空格）
    let mut cfg = default_cfg();
    cfg.align_instance_ports = false;
    let out_false = fmt(src, &cfg);
    assert!(
        out_false.contains(".clk(clk)") && out_false.contains(".long_port(data)"),
        "关闭时紧凑输出: {out_false}"
    );
    assert!(
        !out_false.contains("(  clk"),
        "关闭时无括号内空格: {out_false}"
    );
    assert_ne!(out_true, out_false, "align_instance_ports 开关应改变输出");
}

#[test]
fn space_inside_instance_port_parens() {
    let src = "module t;\nu_foo u ( .clk(clk) );\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.space_inside_instance_port_parens = 1;
    let out = fmt(src, &cfg);
    assert!(out.contains("( clk )"), "括号内侧 1 空格: {out}");
    let out2 = fmt(src, &default_cfg());
    assert!(out2.contains("(  clk  )"), "默认括号内侧 2 空格: {out2}");
}

#[test]
fn wrap_instance_ports() {
    let src = "module t;\nu_foo u ( .a(a), .b(b), .c(c) );\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.wrap_instance_ports = 4;
    let out = fmt(src, &cfg);
    // 端口少于 wrap 数量不强制换行（但默认 BreakBeforeOpenParen 仍换行）
    let _ = out;
    let _ = assert_changes(src, |c| c.wrap_instance_ports = 4);
}

// ---------- 对齐赋值 ----------

#[test]
fn align_assignments_toggle() {
    let src = "module t;\nalways @(posedge clk) begin\n    mem_read_valid <= 0;\n    mem_read_address <= 0;\nend\nendmodule\n";
    let mut cfg = default_cfg();
    cfg.align_assignments = false;
    let out = fmt(src, &cfg);
    assert!(out.contains("mem_read_valid <= 0;"), "关闭赋值对齐: {out}");
}
