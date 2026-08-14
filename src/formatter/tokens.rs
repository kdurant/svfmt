//! Token 工具：叶子节点收集、原文空白提取、显示宽度。

use crate::parser::CstNode;

/// 一个叶子 token：CST 中最小的语法单元（identifier、关键字、运算符、注释等）。
#[derive(Debug, Clone, Copy)]
pub struct Token<'a> {
    /// token 在源码中的字节范围。
    pub byte_range: (usize, usize),
    /// token 文本。
    pub text: &'a str,
    /// node kind（用于间隔规则判断）。
    pub kind: &'static str,
    /// 是否是注释。
    pub is_comment: bool,
}

/// 递归收集节点的所有叶子 token（保持源码顺序）。
pub fn leaf_tokens(node: CstNode<'_>) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    collect(node, &mut out);
    out
}

fn collect<'a, 'tree>(node: CstNode<'tree>, out: &mut Vec<Token<'tree>>)
where
    'tree: 'a,
{
    let children = node.children();
    if children.is_empty() {
        // 叶子
        let (s, e) = {
            let r = node.byte_range();
            (r.start, r.end)
        };
        out.push(Token {
            byte_range: (s, e),
            text: node.text(),
            kind: node.kind(),
            is_comment: node.kind().ends_with("comment"),
        });
    } else {
        for c in children {
            collect(c, out);
        }
    }
}

/// 判断空白字符串中是否包含换行。
pub fn has_newline(ws: &str) -> bool {
    ws.contains('\n')
}

/// 提取两段之间的原文空白。
pub fn whitespace_between(source: &str, start: usize, end: usize) -> &str {
    if end <= start {
        return "";
    }
    &source[start..end.min(source.len())]
}

/// 空白中非换行的部分（行内空格/tab），用于决定是否需要重排。
pub fn inline_whitespace(ws: &str) -> &str {
    if let Some(pos) = ws.find('\n') {
        let after = &ws[pos + 1..];
        // 取最后一个换行之后的部分
        if let Some(last) = after.rfind('\n') {
            return &after[last + 1..];
        }
        return after;
    }
    ws
}

/// 字符显示宽度（CJK 按 2 列计，tab 按 tab_width）。
pub fn display_width(s: &str, tab_width: usize) -> usize {
    let mut width = 0;
    for c in s.chars() {
        if c == '\t' {
            width += tab_width;
        } else if (c as u32) > 0x2e80 {
            width += 2;
        } else {
            width += 1;
        }
    }
    width
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::SvParser;

    fn tokens(src: &str) -> Vec<String> {
        let mut p = SvParser::new().unwrap();
        let tree = p.parse(src).unwrap();
        let toks = leaf_tokens(tree.root_node());
        toks.iter().map(|t| t.text.to_string()).collect()
    }

    #[test]
    fn collects_tokens_in_order() {
        let toks = tokens("module top; endmodule\n");
        assert_eq!(toks, ["module", "top", ";", "endmodule"]);
    }

    #[test]
    fn inline_ws_extracts_trailing_spaces() {
        let ws = "   \n    ";
        assert_eq!(inline_whitespace(ws), "    ");
        assert_eq!(inline_whitespace("   "), "   ");
    }

    #[test]
    fn cjk_width_is_two() {
        assert_eq!(display_width("abc", 4), 3);
        assert_eq!(display_width("注释", 4), 4);
    }
}
