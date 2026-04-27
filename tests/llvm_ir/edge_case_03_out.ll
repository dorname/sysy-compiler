; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  %a = alloca i32, align 4
  store i32 5, i32* %a, align 4
  %a1 = load i32, i32* %a, align 4
  %cmp = icmp sgt i32 %a1, 3
  %cmp_i32 = zext i1 %cmp to i32
  %cond_bool = icmp ne i32 %cmp_i32, 0
  %cond_result = zext i1 %cond_bool to i32
  %if_cond = icmp ne i32 %cond_result, 0
  br i1 %if_cond, label %if_true, label %if_false

if_true:                                          ; preds = %mainEntry
  %a4 = load i32, i32* %a, align 4
  %cmp5 = icmp slt i32 %a4, 10
  %cmp_i326 = zext i1 %cmp5 to i32
  %cond_bool7 = icmp ne i32 %cmp_i326, 0
  %cond_result8 = zext i1 %cond_bool7 to i32
  %if_cond9 = icmp ne i32 %cond_result8, 0
  br i1 %if_cond9, label %if_true2, label %if_false10

if_next:                                          ; preds = %if_next3
  ret i32 0

if_false:                                         ; preds = %mainEntry
  ret i32 3

if_true2:                                         ; preds = %if_true
  ret i32 1

if_next3:                                         ; No predecessors!
  br label %if_next

if_false10:                                       ; preds = %if_true
  ret i32 2
}
