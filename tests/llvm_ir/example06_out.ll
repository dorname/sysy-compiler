; ModuleID = 'module'
source_filename = "module"

@a = global i32 0
@count = global i32 0

define i32 @main() {
mainEntry:
  br label %whileCond

whileCond:                                        ; preds = %if_next, %mainEntry
  %a = load i32, i32* @a, align 4
  %cmp = icmp sle i32 %a, 0
  %cmp_result = zext i1 %cmp to i32
  %cond = icmp ne i32 %cmp_result, 0
  br i1 %cond, label %whileBody, label %whileNext

whileBody:                                        ; preds = %whileCond
  %a1 = load i32, i32* @a, align 4
  %sub_result = sub i32 %a1, 1
  store i32 %sub_result, i32* @a, align 4
  %count = load i32, i32* @count, align 4
  %add_result = add i32 %count, 1
  store i32 %add_result, i32* @count, align 4
  %a2 = load i32, i32* @a, align 4
  %cmp3 = icmp slt i32 %a2, -20
  %cmp_result4 = zext i1 %cmp3 to i32
  %cond5 = icmp ne i32 %cmp_result4, 0
  br i1 %cond5, label %if_true, label %if_next

whileNext:                                        ; preds = %if_true, %whileCond
  %count6 = load i32, i32* @count, align 4
  ret i32 %count6

if_true:                                          ; preds = %whileBody
  br label %whileNext

if_next:                                          ; preds = %whileBody
  br label %whileCond
}
