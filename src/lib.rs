//! svfmt —— SystemVerilog Formatter 库核心。
//!
//! 第一阶段：基于 tree-sitter-systemverilog 解析源码并递归打印 CST。
//!
//! 模块划分遵循设计文档：Parser、CST、Formatter、Configuration、CLI 相互解耦。

pub mod cli;
pub mod config;
pub mod document;
pub mod formatter;
pub mod output;
pub mod parser;
