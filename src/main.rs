#![allow(unused_imports)]
#![allow(dead_code)]

mod check;
mod format;
mod gen_llvm_ir;
mod lexer;
mod riscv_codegen;
mod utils;

use crate::check::Checker;
use crate::gen_llvm_ir::Scanner;
use crate::riscv_codegen::generate_asm;
use clap::{Parser, Subcommand};
use std::env;
use std::fs;
use std::io::{stderr, Write};
use tklog::{Format, LEVEL, LOG};

fn log_init() {
    LOG.set_console(true)
        .set_level(LEVEL::Info)
        .set_format(Format::LevelFlag | Format::Time | Format::ShortFileName)
        .set_cutmode_by_size("tklogsize.txt", 1 << 20, 10, true)
        .set_formatter("{level}{time} {file}:{message}\n");
}

#[derive(Parser)]
#[command(name = "sysy-compiler")]
#[command(about = "SysY 编译器：支持词法分析、格式化、语义检查、LLVM IR 生成和 RISC-V 汇编生成")]
#[command(arg_required_else_help = false)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 兼容模式：直接传入输入文件和输出文件（等同于 gen-asm）
    #[arg(allow_hyphen_values = true)]
    input: Option<String>,

    #[arg(allow_hyphen_values = true)]
    output: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// 词法分析：将 SysY 源码转为 Token 流
    Tokenize { file: String },
    /// 代码格式化：输出格式化后的 SysY 代码
    Fmt { file: String },
    /// 语义检查：执行类型检查和作用域分析
    Check { file: String },
    /// 生成 LLVM IR：将 SysY 编译为 LLVM IR
    GenIr {
        input: String,
        #[arg(short, long)]
        output: String,
    },
    /// 生成 RISC-V 汇编：将 SysY 编译为 RISC-V 汇编
    GenAsm {
        input: String,
        #[arg(short, long)]
        output: String,
    },
}

fn read_file(path: &str) -> String {
    match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("读取文件失败: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_gen_asm(input: &str, output: &str) {
    log_init();
    let source = read_file(input);
    if let Err(e) = generate_asm(&source, output) {
        eprintln!("编译错误: {}", e);
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // 向后兼容：如果没有子命令，但提供了两个位置参数，默认执行 gen-asm
    if args.len() >= 3
        && !args[1].starts_with('-')
        && args[1] != "tokenize"
        && args[1] != "fmt"
        && args[1] != "check"
        && args[1] != "gen-ir"
        && args[1] != "gen-asm"
        && !args[2].starts_with('-')
    {
        run_gen_asm(&args[1], &args[2]);
        return;
    }

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Tokenize { file }) => {
            let input = read_file(&file);
            lexer::tokenize(&input);
        }
        Some(Commands::Fmt { file }) => {
            let input = read_file(&file);
            format::fmt(&input);
        }
        Some(Commands::Check { file }) => {
            let input = read_file(&file);
            let mut writer = stderr();
            let mut checker = Checker::new(&input, &mut writer);
            if let Err(e) = checker.syn_check() {
                eprintln!("语义检查出错: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::GenIr { input, output }) => {
            let source = read_file(&input);
            let scanner = Scanner::new(&source, &output);
            if let Err(e) = scanner.scan_collect() {
                eprintln!("编译错误: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::GenAsm { input, output }) => {
            run_gen_asm(&input, &output);
        }
        None => {
            // 如果没有子命令，但提供了两个位置参数（由 clap 解析）
            if let (Some(input), Some(output)) = (cli.input, cli.output) {
                run_gen_asm(&input, &output);
            } else {
                Cli::parse_from(["sysy-compiler", "--help"]);
            }
        }
    }
}
