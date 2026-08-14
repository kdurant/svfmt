//! 输出层：负责把 CST 打印成可读文本（调试用）。
//!
//! 注意：CST 打印只是调试工具，与 Formatter 的最终输出无关。

pub mod cst_printer;

pub use cst_printer::{PrintOptions, print_cst_to_string};
