//! svfmt 命令行入口。

fn main() {
    if let Err(e) = svfmt::cli::run() {
        eprintln!("错误: {e}");
        std::process::exit(1);
    }
}
