//! 集成测试：对 examples/ 下的真实 SystemVerilog 文件进行解析验证。

use std::path::Path;

use svfmt::parser::SvParser;

fn parse_source(source: &str) -> (bool, usize) {
    let mut parser = SvParser::new().expect("grammar 应可加载");
    let tree = parser.parse(source).expect("解析应成功");
    let has_error = tree.has_error();
    let error_nodes = if has_error {
        svfmt::parser::collect_error_nodes(tree.root_node()).len()
    } else {
        0
    };
    (has_error, error_nodes)
}

#[test]
fn all_example_files_parse_without_error() {
    // core.sv 中模块实例化参数列表含尾随逗号（如 `.THREADS_PER_BLOCK(...),`），
    // tree-sitter-systemverilog 按 IEEE 1800 严格语法将其解析为 ERROR。
    // EDA 工具（VCS/Questa）普遍宽容该写法。Formatter 阶段需按"支持 ERROR node"
    // 原则容忍并尽量原样输出，因此这里对该文件使用已知预期值。
    let known_error_counts = [("core.sv", 2)];

    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut parsed_any = false;
    for entry in std::fs::read_dir(&examples).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e != "sv") {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        // 跳过 *_expected.sv / *_tmp.sv
        if name.ends_with("_expected.sv") || name.ends_with("_tmp.sv") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let (has_error, error_nodes) = parse_source(&source);
        if let Some(&(_, expected)) = known_error_counts.iter().find(|(n, _)| *n == name) {
            assert_eq!(error_nodes, expected, "文件 {name} 的已知 ERROR 数量不匹配");
        } else {
            assert!(!has_error, "文件 {name} 存在 {error_nodes} 个语法错误节点");
        }
        parsed_any = true;
    }
    assert!(parsed_any, "examples 目录下应有可解析的 .sv 文件");
}

#[test]
fn alu_example_has_expected_module_structure() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("alu.sv");
    let source = std::fs::read_to_string(&path).unwrap();
    let mut parser = SvParser::new().unwrap();
    let tree = parser.parse(&source).unwrap();
    let root = tree.root_node();

    let module = root.find_named_child("module_declaration").unwrap();
    assert_eq!(module.text(), &source[module.byte_range()]);

    let header = module.find_named_child("module_ansi_header").unwrap();
    let name = header.child_by_field_name("name").unwrap();
    assert_eq!(name.text(), "alu");

    let port_list = header
        .find_named_child("list_of_port_declarations")
        .unwrap();
    let ports = port_list
        .named_children()
        .into_iter()
        .filter(|c| c.kind() == "ansi_port_declaration")
        .count();
    assert_eq!(ports, 9, "alu 应有 9 个端口");
}

#[test]
fn examples_contain_comments_and_directives() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("alu.sv");
    let source = std::fs::read_to_string(&path).unwrap();
    let mut parser = SvParser::new().unwrap();
    let tree = parser.parse(&source).unwrap();

    let kinds = flatten_kinds(tree.root_node());
    assert!(kinds.contains(&"default_nettype_compiler_directive"));
    assert!(kinds.contains(&"timescale_compiler_directive"));
    assert!(kinds.contains(&"one_line_comment"));
}

fn flatten_kinds(node: svfmt::parser::CstNode<'_>) -> Vec<&'static str> {
    let mut out = vec![node.kind()];
    for child in node.children() {
        out.extend(flatten_kinds(child));
    }
    out
}
