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
  %eq_i32 = zext i1 %ne_tmp to i32
  %cond = icmp ne i32 %eq_i32, 0
  br i1 %cond, label %and_next, label %if_next

if_true:                                          ; preds = %and_next
  ret i32 1

if_next:                                          ; preds = %and_next, %mainEntry
  %a7 = load i32, i32* %a, align 4
  %eq_tmp = icmp eq i32 %a7, 0
  %eq_i328 = zext i1 %eq_tmp to i32
  %cond9 = icmp ne i32 %eq_i328, 0
  br i1 %cond9, label %if_true5, label %or_next

and_next:                                         ; preds = %mainEntry
  %f = call i32 @f()
  %ne_tmp2 = icmp ne i32 %f, 0
  %eq_i323 = zext i1 %ne_tmp2 to i32
  %cond4 = icmp ne i32 %eq_i323, 0
  br i1 %cond4, label %if_true, label %if_next

if_true5:                                         ; preds = %or_next, %if_next
  ret i32 2

if_next6:                                         ; preds = %or_next
  ret i32 3

or_next:                                          ; preds = %if_next
  %f10 = call i32 @f()
  %ne_tmp11 = icmp ne i32 %f10, 0
  %eq_i3212 = zext i1 %ne_tmp11 to i32
  %cond13 = icmp ne i32 %eq_i3212, 0
  br i1 %cond13, label %if_true5, label %if_next6
}
