//! 配置项与文档的一致性测试。
//!
//! `verilog_format.md` 是配置项的权威说明（design.md 第八条要求"所有 Formatter
//! 行为必须通过 Configuration 控制，参考 verilog_format.md"）。配置项与文档
//! 漂移是"静默"的：新增选项不写文档 → 用户不知道；文档写了不存在的键 → 用户
//! 按文档写会报错（或过去被静默忽略）。
//!
//! 这里用 `--dump-config` 的输出来对齐两侧。

use std::collections::BTreeSet;

use svfmt::config::{FormatterConfig, dump_default_toml};

/// `--dump-config` 输出的全部配置键（嵌套表展开为 `表.键`）。
fn config_keys() -> BTreeSet<String> {
    let text = dump_default_toml();
    let value: toml::Value = toml::from_str(&text).expect("dump-config 输出应为合法 TOML");
    let mut out = BTreeSet::new();
    collect_keys(&value, "", &mut out);
    out
}

fn collect_keys(value: &toml::Value, prefix: &str, out: &mut BTreeSet<String>) {
    let toml::Value::Table(table) = value else {
        return;
    };
    for (k, v) in table {
        let path = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        if matches!(v, toml::Value::Table(_)) {
            collect_keys(v, &path, out);
        } else {
            out.insert(path);
        }
    }
}

/// `verilog_format.md` 里的选项标题（`## 选项名`）。
///
/// 纯中文标题（`## 括号`、`## 其他`）是章节分隔，不算选项。
fn documented_keys() -> BTreeSet<String> {
    include_str!("../verilog_format.md")
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .map(str::trim)
        .filter(|s| s.chars().any(|c| c.is_ascii_alphanumeric()))
        .map(str::to_string)
        .collect()
}

#[test]
fn every_config_key_is_documented() {
    let doc = documented_keys();
    let keys = config_keys();
    let missing: Vec<&String> = keys.iter().filter(|k| !doc.contains(*k)).collect();
    assert!(
        missing.is_empty(),
        "以下配置项没有文档（请在 verilog_format.md 添加 `## <选项名>`）: {missing:?}"
    );
}

#[test]
fn every_documented_key_exists_in_config() {
    let keys = config_keys();
    let doc = documented_keys();
    let stale: Vec<&String> = doc.iter().filter(|d| !keys.contains(*d)).collect();
    assert!(
        stale.is_empty(),
        "文档里有配置中不存在的选项（文档过期或拼写错误）: {stale:?}"
    );
}

#[test]
fn config_keys_are_documented_with_defaults() {
    // 每个文档小节都应给出默认值，避免用户只能靠试。
    let doc = include_str!("../verilog_format.md");
    let mut missing_default = Vec::new();
    for key in config_keys() {
        let Some(pos) = doc.find(&format!("## {key}\n")) else {
            continue; // 由 every_config_key_is_documented 报错
        };
        let rest = &doc[pos..];
        let section = match rest[3..].find("\n## ") {
            Some(end) => &rest[..end + 3],
            None => rest,
        };
        if !section.contains("默认") {
            missing_default.push(key);
        }
    }
    assert!(
        missing_default.is_empty(),
        "以下选项的文档未标注默认值: {missing_default:?}"
    );
}

#[test]
fn dump_config_matches_default_struct() {
    // `--dump-config` 输出的值必须能被解析回默认配置（等价性再校验一次）。
    let cfg: FormatterConfig = toml::from_str(&dump_default_toml()).expect("应可解析");
    assert_eq!(cfg.indent_width, FormatterConfig::default().indent_width);
    assert_eq!(cfg.column_limit, FormatterConfig::default().column_limit);
    assert_eq!(
        cfg.end_of_line,
        FormatterConfig::default().end_of_line,
        "新增选项需同步 Default 实现"
    );
}
