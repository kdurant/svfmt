//! CLI 集成测试：多文件、glob 展开、`--in-place`、stdin 等命令行行为。
//!
//! 通过 `CARGO_BIN_EXE_svfmt` 调用编译出的二进制，验证真实命令行行为。
//! 注意：`Command` 传参不经过 shell，因此 glob 模式不会被 shell 展开，
//! 正好用于验证程序内部的 glob 展开逻辑。

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn svfmt_bin() -> &'static str {
    env!("CARGO_BIN_EXE_svfmt")
}

/// 创建唯一临时目录，测试结束后由 OS 清理。
fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("svfmt-cli-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 用 Formatter 库函数计算期望输出，与二进制结果对比。
fn expected(src: &str) -> String {
    svfmt::formatter::Formatter::format_source(src, &svfmt::config::FormatterConfig::default())
        .unwrap()
}

#[test]
fn in_place_multiple_files() {
    let dir = temp_dir("multi");
    let a = dir.join("a.sv");
    let b = dir.join("b.sv");
    let src_a = "module a(input logic clk);endmodule\n";
    let src_b = "module b(input logic rst);endmodule\n";
    std::fs::write(&a, src_a).unwrap();
    std::fs::write(&b, src_b).unwrap();

    let out = Command::new(svfmt_bin())
        .arg("--in-place")
        .arg(&a)
        .arg(&b)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert_eq!(std::fs::read_to_string(&a).unwrap(), expected(src_a));
    assert_eq!(std::fs::read_to_string(&b).unwrap(), expected(src_b));
}

#[test]
fn glob_expansion_in_place() {
    let dir = temp_dir("glob");
    for name in ["x.sv", "y.sv", "z.sv"] {
        std::fs::write(dir.join(name), "module m();endmodule\n").unwrap();
    }
    // 非 .sv 文件不应被匹配
    std::fs::write(dir.join("note.txt"), "hello").unwrap();

    let pattern = dir.join("*.sv");
    let out = Command::new(svfmt_bin())
        .arg("--in-place")
        .arg(pattern.to_str().unwrap())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    for name in ["x.sv", "y.sv", "z.sv"] {
        let content = std::fs::read_to_string(dir.join(name)).unwrap();
        assert_eq!(
            content,
            expected("module m();endmodule\n"),
            "{name} 应被格式化"
        );
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("note.txt")).unwrap(),
        "hello",
        "非 .sv 文件不应被修改"
    );
}

#[test]
fn glob_no_match_is_error() {
    let dir = temp_dir("globnomatch");
    let pattern = dir.join("*.sv");
    let out = Command::new(svfmt_bin())
        .arg("--in-place")
        .arg(pattern.to_str().unwrap())
        .output()
        .unwrap();
    assert!(!out.status.success(), "glob 无匹配应失败");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("没有匹配"), "stderr: {stderr}");
}

#[test]
fn output_flag_conflicts_with_multiple_files() {
    let dir = temp_dir("outconflict");
    let a = dir.join("a.sv");
    let b = dir.join("b.sv");
    std::fs::write(&a, "module a();endmodule\n").unwrap();
    std::fs::write(&b, "module b();endmodule\n").unwrap();

    let out = Command::new(svfmt_bin())
        .arg("-o")
        .arg(dir.join("out.sv"))
        .arg(&a)
        .arg(&b)
        .output()
        .unwrap();
    assert!(!out.status.success(), "-o 与多文件应冲突");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("冲突"), "stderr: {stderr}");
}

#[test]
fn missing_file_does_not_stop_others() {
    let dir = temp_dir("partial");
    let a = dir.join("a.sv");
    let missing = dir.join("missing.sv");
    std::fs::write(&a, "module a();endmodule\n").unwrap();

    let out = Command::new(svfmt_bin())
        .arg("--in-place")
        .arg(&missing)
        .arg(&a)
        .output()
        .unwrap();
    assert!(!out.status.success(), "存在失败文件时退出码应为非 0");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("missing.sv"),
        "stderr 应包含失败文件: {stderr}"
    );

    // a.sv 仍应被格式化
    assert_eq!(
        std::fs::read_to_string(&a).unwrap(),
        expected("module a();endmodule\n")
    );
}

#[test]
fn stdin_stdout_still_works() {
    use std::io::Write;

    let mut child = Command::new(svfmt_bin())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"module a();endmodule\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout, expected("module a();endmodule\n"));
}

#[test]
fn single_file_with_output_flag() {
    let dir = temp_dir("singleout");
    let a = dir.join("a.sv");
    let out_path = dir.join("out.sv");
    std::fs::write(&a, "module a();endmodule\n").unwrap();

    let out = Command::new(svfmt_bin())
        .arg("-o")
        .arg(&out_path)
        .arg(&a)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&out_path).unwrap(),
        expected("module a();endmodule\n")
    );
    // 输入文件保持不变
    assert_eq!(
        std::fs::read_to_string(&a).unwrap(),
        "module a();endmodule\n"
    );
}

#[test]
fn cst_flag_works_with_multiple_files() {
    let dir = temp_dir("cstmulti");
    let a = dir.join("a.sv");
    let b = dir.join("b.sv");
    std::fs::write(&a, "module a();endmodule\n").unwrap();
    std::fs::write(&b, "module b();endmodule\n").unwrap();

    let out = Command::new(svfmt_bin())
        .arg("--cst")
        .arg(&a)
        .arg(&b)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("module_declaration"), "stdout: {stdout}");
}

#[test]
fn cli_override_column_limit() {
    let src = "\
module t;
initial begin
  $display(\"long format string %d %d\", a, b, c, d, e, f, g, h, i, j);
end
endmodule
";
    let dir = temp_dir("override");
    let a = dir.join("a.sv");
    std::fs::write(&a, src).unwrap();

    // 默认（column_limit=100）：该行 70 列，未超限 → 不断行
    let out = Command::new(svfmt_bin()).arg(&a).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("$display(\"long format string %d %d\", a, b, c, d, e, f, g, h, i, j);"),
        "默认应保持单行: {stdout}"
    );

    // --column-limit 40：超宽函数调用断行
    let out = Command::new(svfmt_bin())
        .arg("--column-limit")
        .arg("40")
        .arg(&a)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("$display(\n"),
        "--column-limit 40 应断行: {stdout}"
    );
}

#[test]
fn cli_override_precedence_over_config_file() {
    let src = "\
module t;
initial begin
  $display(\"long format string %d %d\", a, b, c, d, e, f, g, h, i, j);
end
endmodule
";
    let dir = temp_dir("precedence");
    let a = dir.join("a.sv");
    let cfg = dir.join("svfmt.toml");
    std::fs::write(&a, src).unwrap();
    std::fs::write(&cfg, "column_limit = 40\n").unwrap();

    // 配置文件 column_limit=40 → 断行
    let out = Command::new(svfmt_bin())
        .arg("--config")
        .arg(&cfg)
        .arg(&a)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("$display(\n"),
        "配置文件 column_limit=40 应断行: {stdout}"
    );

    // CLI 覆盖 --column-limit 0 → 保持单行（CLI 优先级最高）
    let out = Command::new(svfmt_bin())
        .arg("--config")
        .arg(&cfg)
        .arg("--column-limit")
        .arg("0")
        .arg(&a)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("$display(\"long format string %d %d\", a, b, c, d, e, f, g, h, i, j);"),
        "CLI --column-limit 0 应覆盖配置文件: {stdout}"
    );
}

#[test]
fn cli_override_bool_flags() {
    let src = "\
module t;
logic a;
initial begin
  if (a) b <= 1;
end
endmodule
";
    let dir = temp_dir("boolflags");
    let a = dir.join("a.sv");
    std::fs::write(&a, src).unwrap();

    // --use-tab 裸用（true）：缩进为 Tab
    let out = Command::new(svfmt_bin())
        .arg("--use-tab")
        .arg(&a)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains('\t'),
        "--use-tab 应使用 Tab 缩进: {:?}",
        stdout
    );

    // --use-tab=false：仍为空格
    let out = Command::new(svfmt_bin())
        .arg("--use-tab=false")
        .arg(&a)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains('\t'),
        "--use-tab=false 不应使用 Tab: {:?}",
        stdout
    );

    // --around-assignment=false：赋值符号两侧无空格
    let src2 = "module t; assign y = a + b; endmodule\n";
    std::fs::write(&a, src2).unwrap();
    let out = Command::new(svfmt_bin())
        .arg("--around-assignment=false")
        .arg(&a)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("y =a + b;"),
        "--around-assignment=false 应去掉 = 后空格: {stdout}"
    );
}

#[test]
fn cli_override_reformat_case() {
    let src = "\
module t;
logic a;
initial begin
  casez(a)
  1 : b = 1;
  endcase
end
endmodule
";
    let dir = temp_dir("reformatcase");
    let a = dir.join("a.sv");
    std::fs::write(&a, src).unwrap();

    // --reformat-case casex：统一为 casex
    let out = Command::new(svfmt_bin())
        .arg("--reformat-case")
        .arg("casex")
        .arg(&a)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("casex(a)"),
        "--reformat-case casex 应统一为 casex: {stdout}"
    );
    assert!(!stdout.contains("casez(a)"), "不应残留 casez: {stdout}");
}
