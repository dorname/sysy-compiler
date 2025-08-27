use pest::Parser;
use pest_derive::Parser;
#[derive(Parser)]
#[grammar = "lexer.pest"]
pub struct ExpressionParser;
#[derive(Debug, PartialEq)]
pub enum Token {
    Integer(i64),
    Operator(char),
}
pub fn tokenize(input: &str)  {
    let pair = ExpressionParser::parse(Rule::file, input)
        .unwrap_or_else(|e| panic!("Parse error: {}", e))
        .next()
        .unwrap(); // 取第一个匹配
    pair.into_inner()
        .for_each(|p| {
            // match p.as_rule() {
            //     Rule::operator =
            // }
          eprintln!("{:?}:{:?}", p.as_rule(),p);
         });
    // eprintln!("{:?}", pair);
}

#[cfg(test)]
mod tests {
    use crate::BASE_PATH;
    use super::*;
    #[test]
    fn test_tokenize() {
        let filename = BASE_PATH.to_string() + "lab1_example1.sysy";

        let file = std::fs::read_to_string(filename).expect("Failed to read file");

        tokenize(&file);
    }
}