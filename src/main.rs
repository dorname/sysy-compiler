#![allow(unused_imports)]
#![allow(dead_code)]

mod utils;
mod gen_llvm_ir;

use crate::gen_llvm_ir::*;
use std::{env, fs};
use std::io::{stderr, stdout};



fn main() {
    // 收集命令行参数
    let args: Vec<String> = env::args().collect();

    // 检查是否提供了文件名
    if args.len() < 3 {
        eprintln!("Usage: {} <filename>,{}<output>", args[0],args[1]);
        std::process::exit(1);
    }

    // 获取文件名
    let filename = &args[1];

    // 获取输出文件名
    let output = &args[2];

    // 读取输入文件
    let input = match fs::read_to_string(filename) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("读取文件失败: {}", e);
            std::process::exit(1);
        }
    };
    
    let scanner = Scanner::new(&input, output);
    if let Err(e) = scanner.scan_collect() {
        eprintln!("编译错误: {}", e);
        std::process::exit(1);
    }

}
