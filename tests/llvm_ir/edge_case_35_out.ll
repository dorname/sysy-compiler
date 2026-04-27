; ModuleID = 'module'
source_filename = "module"

define i32 @compare(i32 %a, i32 %b) {
compareEntry:
  %a2 = alloca i32, align 4
  store i32 %a, i32* %a2, align 4
  %b4 = alloca i32, align 4
  store i32 %b, i32* %b4, align 4
  %a5 = load i32, i32* %a2, align 4
  %b6 = load i32, i32* %b4, align 4
  %cmp = icmp sgt i32 %a5, %b6
  %cmp_i32 = zext i1 %cmp to i32
  %cond_bool = icmp ne i32 %cmp_i32, 0
  %cond_result = zext i1 %cond_bool to i32
  %if_cond = icmp ne i32 %cond_result, 0
  br i1 %if_cond, label %if_true, label %if_false

if_true:                                          ; preds = %compareEntry
  ret i32 1

if_next:                                          ; preds = %if_next8
  ret i32 0

if_false:                                         ; preds = %compareEntry
  %a9 = load i32, i32* %a2, align 4
  %b10 = load i32, i32* %b4, align 4
  %cmp11 = icmp slt i32 %a9, %b10
  %cmp_i3212 = zext i1 %cmp11 to i32
  %cond_bool13 = icmp ne i32 %cmp_i3212, 0
  %cond_result14 = zext i1 %cond_bool13 to i32
  %if_cond15 = icmp ne i32 %cond_result14, 0
  br i1 %if_cond15, label %if_true7, label %if_false16

if_true7:                                         ; preds = %if_false
  ret i32 -1

if_next8:                                         ; No predecessors!
  br label %if_next

if_false16:                                       ; preds = %if_false
  ret i32 0
}

define i32 @main() {
mainEntry:
  %x = alloca i32, align 4
  store i32 5, i32* %x, align 4
  %y = alloca i32, align 4
  store i32 3, i32* %y, align 4
  %z = alloca i32, align 4
  store i32 7, i32* %z, align 4
  %x1 = load i32, i32* %x, align 4
  %y2 = load i32, i32* %y, align 4
  %compare = call i32 @compare(i32 %x1, i32 %y2)
  %result1 = alloca i32, align 4
  store i32 %compare, i32* %result1, align 4
  %y3 = load i32, i32* %y, align 4
  %z4 = load i32, i32* %z, align 4
  %compare5 = call i32 @compare(i32 %y3, i32 %z4)
  %result2 = alloca i32, align 4
  store i32 %compare5, i32* %result2, align 4
  %result16 = load i32, i32* %result1, align 4
  %cmp = icmp sgt i32 %result16, 0
  %cmp_i32 = zext i1 %cmp to i32
  %cond_bool = icmp ne i32 %cmp_i32, 0
  %cond_result = zext i1 %cond_bool to i32
  %if_cond = icmp ne i32 %cond_result, 0
  br i1 %if_cond, label %if_true, label %if_false

if_true:                                          ; preds = %mainEntry
  %result29 = load i32, i32* %result2, align 4
  %cmp10 = icmp slt i32 %result29, 0
  %cmp_i3211 = zext i1 %cmp10 to i32
  %cond_bool12 = icmp ne i32 %cmp_i3211, 0
  %cond_result13 = zext i1 %cond_bool12 to i32
  %if_cond14 = icmp ne i32 %cond_result13, 0
  br i1 %if_cond14, label %if_true7, label %if_false15

if_next:                                          ; preds = %if_next8
  ret i32 0

if_false:                                         ; preds = %mainEntry
  ret i32 3

if_true7:                                         ; preds = %if_true
  ret i32 1

if_next8:                                         ; No predecessors!
  br label %if_next

if_false15:                                       ; preds = %if_true
  ret i32 2
}
