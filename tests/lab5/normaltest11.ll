@gg = global i32 5, align 4
define i32 @main() {
entry:
%b = alloca i32, align 4
store i32 12, i32* %b, align 4
%0 = load i32, i32* @gg, align 4
%mul = mul i32 12, %0
ret i32 %mul
}