mod lexer;
mod utils;
use lexer::*;

const BASE_PATH:&str = "tests/";
fn main() {
    // let args:Vec<String> = std::env::args().collect();
    // if args.len() < 2 {
    //     eprintln!("Usage: {} <filename>", args[0]);
    //     std::process::exit(1);
    // }
    //
    // let filename = args[1].to_string()+BASE_PATH;
    let filename = BASE_PATH.to_string() + "lab1_example1.sysy";

    let file = std::fs::read_to_string(filename).expect("Failed to read file");
    
    tokenize(&file);
}
