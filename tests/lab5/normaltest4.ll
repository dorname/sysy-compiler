define i32 @defn() {
entry:
ret i32 4
}
define i32 @main() {
entry:
%a = alloca i32, align 4
%call = call i32 @defn()
store i32 %call, i32* %a, align 4
%0 = load i32, i32* %a, align 4
ret i32 %0
}