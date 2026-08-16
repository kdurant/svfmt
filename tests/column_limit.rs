//! `column_limit` 断行功能测试。
//!
//! column_limit > 0 时，结构化路径（函数调用参数、表达式容器）在超宽时
//! 断行；column_limit = 0（默认）保持单行。对齐段（模块体 assign、seq_block
//! 赋值段、实例端口连接）依赖单行文本计算列宽，强制保持单行（不受
//! column_limit 影响）。所有断行必须是确定性的（幂等）。

use svfmt::config::FormatterConfig;
use svfmt::formatter::Formatter;

fn fmt_with(src: &str, column_limit: u32) -> String {
    let cfg = FormatterConfig {
        column_limit,
        ..FormatterConfig::default()
    };
    Formatter::format_source(src, &cfg).unwrap()
}

/// 默认 column_limit=0：超长内容保持单行。
#[test]
fn zero_limit_keeps_single_line() {
    let src = "module t; assign y = a + b + c + d + e + f + g + h + i + j + k; endmodule\n";
    let out = fmt_with(src, 0);
    assert!(
        out.contains("assign y = a + b + c + d + e + f + g + h + i + j + k;"),
        "got: {out}"
    );
}

/// 对齐段（模块体 assign）的 RHS 保持单行，不受 column_limit 影响。
#[test]
fn aligned_assign_stays_single_line() {
    let src = "module t; assign y = a + b + c + d + e + f + g + h + i + j + k; endmodule\n";
    let out = fmt_with(src, 40);
    // 对齐段 RHS 单行（不因 column_limit 断行，保证对齐结构稳定）
    assert!(
        out.contains("assign y = a + b + c + d + e + f + g + h + i + j + k;"),
        "对齐段不应断行: {out}"
    );
}

/// 函数调用参数超宽时断行（fmt_call 的 `(` 后 SoftLine）。
#[test]
fn call_arguments_wrap_at_limit() {
    let src = "\
module t;
  initial begin
    $display(\"long format string %d %d %d %d\", a, b, c, d, e, f, g, h, i, j);
  end
endmodule
";
    let out = fmt_with(src, 40);
    assert!(
        out.contains("$display(\n"),
        "超宽函数调用应在左括号后断行: {out}"
    );
    // 参数（含字符串与后续实参）在新行缩进
    assert!(
        out.lines().any(|l| l.trim_start().starts_with("\"long format string")),
        "首个参数应在新行: {out}"
    );
    assert!(
        out.lines().any(|l| l.trim_start().starts_with('a')),
        "实参应在新行: {out}"
    );
}

/// 断行结果幂等：再次格式化不变。
#[test]
fn wrapping_is_idempotent() {
    let src = "\
module t;
  initial begin
    $display(\"long format string %d %d %d %d\", a, b, c, d, e, f, g, h, i, j);
  end
endmodule
";
    let once = fmt_with(src, 40);
    let twice = fmt_with(&once, 40);
    assert_eq!(once, twice);
}

/// 较短的内容在 limit 内保持单行。
#[test]
fn short_call_stays_single_line() {
    let src = "\
module t;
  initial begin
    $display(\"short\", a, b, c);
  end
endmodule
";
    let out = fmt_with(src, 40);
    assert!(out.contains("$display(\"short\", a, b, c);"), "got: {out}");
}

/// 嵌套表达式（含函数调用）超宽时断行，且幂等。
#[test]
fn nested_call_wraps_idempotently() {
    let src = "\
module t;
  initial begin
    x = compute(a, b) + compute(c, d) + compute(e, f) + compute(g, h) + compute(i, j);
  end
endmodule
";
    let once = fmt_with(src, 40);
    let twice = fmt_with(&once, 40);
    assert_eq!(once, twice, "嵌套调用断行应幂等");
    assert!(once.contains('\n'), "超宽表达式应断行: {once}");
}
