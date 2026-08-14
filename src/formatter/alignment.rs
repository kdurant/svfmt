//! 对齐引擎：把多行内容按列对齐。
//!
//! 输入每行的列内容（`Vec<String>`），每列按最大显示宽度补齐，
//! 列之间用固定数量的空格分隔。

use crate::formatter::tokens::display_width;

/// 对齐行组。
///
/// `rows` 的每一行是若干列内容；行内列数可以不同（缺失列视为空）。
/// 返回对齐后的行。
pub fn align_rows(rows: &[Vec<String>], tab_width: usize) -> Vec<String> {
    if rows.is_empty() {
        return Vec::new();
    }
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return rows.iter().map(|r| r.join("")).collect();
    }
    // 计算每列最大宽度
    let mut col_widths = vec![0usize; col_count];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            let w = display_width(cell, tab_width);
            if w > col_widths[i] {
                col_widths[i] = w;
            }
        }
    }
    // 末列不填充
    col_widths[col_count - 1] = 0;
    rows.iter()
        .map(|row| {
            let mut out = String::new();
            for (i, cell) in row.iter().enumerate() {
                out.push_str(cell);
                if i + 1 < row.len() && i + 1 < col_count {
                    let pad = col_widths[i].saturating_sub(display_width(cell, tab_width)) + 1;
                    for _ in 0..pad {
                        out.push(' ');
                    }
                }
            }
            out
        })
        .collect()
}

/// 把一列内容补空格到指定宽度（用于单个行尾对齐，如注释对齐）。
pub fn pad_to(text: &str, width: usize, tab_width: usize) -> String {
    let w = display_width(text, tab_width);
    if w >= width {
        return text.to_string();
    }
    let mut out = String::from(text);
    for _ in 0..(width - w) {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_simple_columns() {
        let rows = vec![
            vec!["input".into(), "wire".into(), "clk".into()],
            vec!["output".into(), "logic".into(), "data".into()],
        ];
        let out = align_rows(&rows, 4);
        assert_eq!(out[0], "input  wire  clk");
        assert_eq!(out[1], "output logic data");
    }

    #[test]
    fn missing_cells_are_empty() {
        let rows = vec![vec!["a".into(), "b".into()], vec!["long".into()]];
        let out = align_rows(&rows, 4);
        assert_eq!(out[0], "a    b");
        assert_eq!(out[1], "long");
    }

    #[test]
    fn pad_to_fills_spaces() {
        assert_eq!(pad_to("ab", 6, 4), "ab    ");
        assert_eq!(pad_to("abcdef", 4, 4), "abcdef");
    }
}
