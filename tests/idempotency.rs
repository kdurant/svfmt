//! 幂等性 golden 测试：examples 下每个输入格式化后应与 `*_expected.sv` 一致，
//! 且重复格式化结果保持不变（幂等）。
//!
//! 幂等性是格式化工具正确性的根基：`format(format(x)) == format(x)`。
//! 若某示例不幂等（解析结构随空白漂移、缩进累积等），这里会直接失败。

use std::path::Path;

use svfmt::config::FormatterConfig;
use svfmt::formatter::Formatter;

fn fmt(src: &str) -> String {
    Formatter::format_source(src, &FormatterConfig::default()).unwrap()
}

/// 每个 example：输出与 `*_expected.sv` 一致，且二次格式化不变。
#[test]
fn examples_match_expected_and_are_idempotent() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut checked = 0usize;
    let mut entries: Vec<_> = std::fs::read_dir(&examples)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();

    for path in entries {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        if !name.ends_with(".sv") || name.ends_with("_expected.sv") || name.ends_with("_tmp.sv") {
            continue;
        }
        let stem = name.trim_end_matches(".sv");
        let expected_path = examples.join(format!("{stem}_expected.sv"));
        if !expected_path.exists() {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();

        // ① 与 golden 一致
        let once = fmt(&src);
        let expected = std::fs::read_to_string(&expected_path).unwrap();
        assert_eq!(once, expected, "文件 {name} 的输出与 {stem}_expected.sv 不一致");

        // ② 幂等：二次格式化输出不变
        let twice = fmt(&once);
        assert_eq!(once, twice, "文件 {name} 不幂等：二次格式化结果发生变化");

        checked += 1;
    }

    assert!(
        checked >= 9,
        "应至少检查 9 个带 golden 的示例，实际仅 {checked}"
    );
}

/// 在 column_limit 开启时，examples 也必须保持幂等（断行是确定性的）。
#[test]
fn examples_are_idempotent_with_column_limit() {
    let cfg = FormatterConfig {
        column_limit: 80,
        ..FormatterConfig::default()
    };
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut checked = 0usize;

    for entry in std::fs::read_dir(&examples).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        if !name.ends_with(".sv")
            || name.ends_with("_expected.sv")
            || name.ends_with("_tmp.sv")
        {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let once = Formatter::format_source(&src, &cfg).unwrap();
        let twice = Formatter::format_source(&once, &cfg).unwrap();
        assert_eq!(once, twice, "文件 {name} 在 column_limit=80 下不幂等");
        checked += 1;
    }

    assert!(checked >= 9, "应至少检查 9 个示例，实际仅 {checked}");
}
