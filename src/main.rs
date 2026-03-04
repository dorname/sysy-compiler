#![allow(unused_imports)]
#![allow(dead_code)]

mod gen_llvm_ir;
mod riscv_codegen;

use crate::gen_llvm_ir::*;
use std::{env, fs};
use std::io::{stderr, stdout};
use crate::riscv_codegen::generate_asm;

use tklog::{
    sync::Logger,LEVEL, LOG,
    Format,MODE,
};

fn log_init() {
    LOG.set_console(true)
        .set_level(LEVEL::Info)
        .set_format(Format::LevelFlag | Format::Time | Format::ShortFileName)
        .set_cutmode_by_size("tklogsize.txt", 1<<20, 10, true)
        .set_formatter("{level}{time} {file}:{message}\n");
}

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

    let _ = generate_asm(&input, &output);
}
