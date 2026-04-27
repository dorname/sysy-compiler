use num_bigint::BigInt;
use num_traits::Num;

pub fn hex_to_int(s: &str) -> Result<BigInt, num_bigint::ParseBigIntError> {
    // 去空格
    let mut input = s.trim();
    // 去负号
    input = if input.starts_with('-') {
        &input[1..]
    } else {
        input
    };
    // 去掉前缀0x | 0X
    input = if input.starts_with("0x") || input.starts_with("0X") {
        &input[2..]
    } else {
        input
    };
    BigInt::from_str_radix(input, 16)
}

pub fn oct_to_int(s: &str) -> Result<BigInt, num_bigint::ParseBigIntError> {
    if s == "0" {
        return Ok(BigInt::from(0));
    }
    // 去空格
    let mut input = s.trim();
    // 去负号
    input = if input.starts_with('-') {
        &input[1..]
    } else {
        input
    };
    // 去掉前缀0
    input = if input.starts_with("0") || input.starts_with("0") {
        &input[1..]
    } else {
        input
    };
    BigInt::from_str_radix(input, 8)
}

pub fn add_option_string(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(a_str), Some(b_str)) => Some(format!("{}{}", a_str, b_str)),
        (Some(a_str), None) => Some(a_str.to_string()),
        (None, Some(b_str)) => Some(b_str.to_string()),
        (None, None) => None,
    }
}

pub fn eq_option_string(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(a_str), Some(b_str)) => a_str == b_str,
        (None, None) => true,
        _ => false,
    }
}
