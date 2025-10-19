; ModuleID = 'module'
source_filename = "module"

define i32 @f() {
fEntry:
  ret i32 1
}

define i32 @main() {
mainEntry:
  %a = alloca i32, align 4
  store i32 0, i32* %a, align 4
  %a1 = load i32, i32* %a, align 4
  %ne_tmp = icmp ne i32 %a1, 0
  %f = call i32 @f()
  %ne_tmp2 = icmp ne i32 %f, 0
  %and_tmp = and i1 %ne_tmp, %ne_tmp2
  br i1 %and_tmp, label %if_true, label %if_next

if_true:                                          ; preds = %mainEntry
  ret i32 1

if_next:                                          ; preds = %mainEntry
  %a5 = load i32, i32* %a, align 4
  %eq_tmp = icmp eq i32 %a5, 0
  %f6 = call i32 @f()
  %ne_tmp7 = icmp ne i32 %f6, 0
  %or_tmp = or i1 %eq_tmp, %ne_tmp7
  br i1 %or_tmp, label %if_true3, label %if_next4

if_true3:                                         ; preds = %if_next
  ret i32 2

if_next4:                                         ; preds = %if_next
  ret i32 3
}
