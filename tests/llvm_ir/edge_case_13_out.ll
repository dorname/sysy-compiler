; ModuleID = 'module'
source_filename = "module"

define i32 @factorial(i32 %n) {
factorialEntry:
  %n2 = alloca i32, align 4
  store i32 %n, i32* %n2, align 4
  %n3 = load i32, i32* %n2, align 4
  %cmp = icmp sle i32 %n3, 1
  br i1 %cmp, label %if_true, label %if_false

if_true:                                          ; preds = %factorialEntry
  ret i32 1

if_next:                                          ; No predecessors!
  ret i32 0

if_false:                                         ; preds = %factorialEntry
  %n4 = load i32, i32* %n2, align 4
  %n5 = load i32, i32* %n2, align 4
  %sub_tmp = sub i32 %n5, 1
  %factorial = call i32 @factorial(i32 %sub_tmp)
  %mul_tmp = mul i32 %n4, %factorial
  ret i32 %mul_tmp
}

define i32 @main() {
mainEntry:
  %factorial = call i32 @factorial(i32 4)
  ret i32 %factorial
}
