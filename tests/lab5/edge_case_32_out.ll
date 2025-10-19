; ModuleID = 'module'
source_filename = "module"

define i32 @f1() {
f1Entry:
  ret i32 1
}

define i32 @f2() {
f2Entry:
  ret i32 0
}

define i32 @main() {
mainEntry:
  %a = alloca i32, align 4
  store i32 0, i32* %a, align 4
  %b = alloca i32, align 4
  store i32 1, i32* %b, align 4
  %a1 = load i32, i32* %a, align 4
  %to_bool = icmp ne i32 %a1, 0
  %ne_tmp = icmp ne i1 %to_bool, false
  %f1 = call i32 @f1()
  %to_bool2 = icmp ne i32 %f1, 0
  %ne_tmp3 = icmp ne i1 %to_bool2, false
  %and_bool = and i1 %ne_tmp, %ne_tmp3
  br i1 %and_bool, label %if_true, label %if_next

if_true:                                          ; preds = %mainEntry
  ret i32 1

if_next:                                          ; preds = %mainEntry
  %b6 = load i32, i32* %b, align 4
  %to_bool7 = icmp ne i32 %b6, 0
  %ne_tmp8 = icmp ne i1 %to_bool7, false
  %f2 = call i32 @f2()
  %to_bool9 = icmp ne i32 %f2, 0
  %ne_tmp10 = icmp ne i1 %to_bool9, false
  %or_bool = or i1 %ne_tmp8, %ne_tmp10
  br i1 %or_bool, label %if_true4, label %if_next5

if_true4:                                         ; preds = %if_next
  ret i32 2

if_next5:                                         ; preds = %if_next
  %a13 = load i32, i32* %a, align 4
  %to_bool14 = icmp ne i32 %a13, 0
  %eq_tmp = icmp eq i1 %to_bool14, false
  %b15 = load i32, i32* %b, align 4
  %to_bool16 = icmp ne i32 %b15, 0
  %ne_tmp17 = icmp ne i1 %to_bool16, false
  %and_bool18 = and i1 %eq_tmp, %ne_tmp17
  br i1 %and_bool18, label %if_true11, label %if_next12

if_true11:                                        ; preds = %if_next5
  ret i32 3

if_next12:                                        ; preds = %if_next5
  ret i32 4
}
