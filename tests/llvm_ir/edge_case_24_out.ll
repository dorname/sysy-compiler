; ModuleID = 'module'
source_filename = "module"

define i32 @fibonacci(i32 %n) {
fibonacciEntry:
  %n2 = alloca i32, align 4
  store i32 %n, i32* %n2, align 4
  %n3 = load i32, i32* %n2, align 4
  %cmp = icmp sle i32 %n3, 1
  br i1 %cmp, label %if_true, label %if_false

if_true:                                          ; preds = %fibonacciEntry
  %n4 = load i32, i32* %n2, align 4
  ret i32 %n4

if_next:                                          ; No predecessors!
  ret i32 0

if_false:                                         ; preds = %fibonacciEntry
  %n5 = load i32, i32* %n2, align 4
  %sub_tmp = sub i32 %n5, 1
  %fibonacci = call i32 @fibonacci(i32 %sub_tmp)
  %n6 = load i32, i32* %n2, align 4
  %sub_tmp7 = sub i32 %n6, 2
  %fibonacci8 = call i32 @fibonacci(i32 %sub_tmp7)
  %add_tmp = add i32 %fibonacci, %fibonacci8
  ret i32 %add_tmp
}

define i32 @main() {
mainEntry:
  %fibonacci = call i32 @fibonacci(i32 6)
  ret i32 %fibonacci
}
