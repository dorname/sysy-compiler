pub fn experiment(
    input1: i32,
    input2: i32,
) -> i32 {
    let a = input1 + input2 + 3;
    let b = input2 - 1;
    if b > 1 {
        return a + b;
    }
    else {
        return 0;
    }
}