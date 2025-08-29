mod lexer;
mod utils;
use lexer::*;
use std::{env, fs};
fn main() {
    // 收集命令行参数
    let args: Vec<String> = env::args().collect();

    // 检查是否提供了文件名
    if args.len() < 2 {
        eprintln!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }

    // 获取文件名
    let filename = &args[1];

    // 读取输入文件
    let input = fs::read_to_string(filename).expect("Failed to read file");
    
    tokenize(&input);
}
