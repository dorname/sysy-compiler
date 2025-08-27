use std::ops::Add;
use clap::builder::Str;
use pest_derive::Parser;
#[derive(Parser)]
#[grammar = "src/lexer.pest"]
struct SysYLexer;

#[cfg(test)]
const BASE_PATH:&str = "../src/";
#[cfg(not(test))]
const BASE_PATH:&str = "";
fn main() {
    let args:Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }

    let filename = args[1].to_string()+BASE_PATH;

    let file = std::fs::read_to_string(filename).expect("Failed to read file");
    
    
}
