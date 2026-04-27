; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  %i = alloca i32, align 4
  store i32 0, i32* %i, align 4
  %sum = alloca i32, align 4
  store i32 0, i32* %sum, align 4
  br label %whileCond

whileCond:                                        ; preds = %if_next6, %mainEntry
  %i1 = load i32, i32* %i, align 4
  %cmp = icmp slt i32 %i1, 10
  %cmp_result = zext i1 %cmp to i32
  %cond = icmp ne i32 %cmp_result, 0
  br i1 %cond, label %whileBody, label %whileNext

whileBody:                                        ; preds = %whileCond
  %i2 = load i32, i32* %i, align 4
  %add_result = add i32 %i2, 1
  store i32 %add_result, i32* %i, align 4
  %i3 = load i32, i32* %i, align 4
  %eq_cmp = icmp eq i32 %i3, 3
  %eq_result = zext i1 %eq_cmp to i32
  %cond4 = icmp ne i32 %eq_result, 0
  br i1 %cond4, label %if_true, label %if_next

whileNext:                                        ; preds = %whileCond
  %sum14 = load i32, i32* %sum, align 4
  ret i32 %sum14

if_true:                                          ; preds = %whileBody
  br label %if_next

if_next:                                          ; preds = %if_true, %whileBody
  %i7 = load i32, i32* %i, align 4
  %eq_cmp8 = icmp eq i32 %i7, 7
  %eq_result9 = zext i1 %eq_cmp8 to i32
  %cond10 = icmp ne i32 %eq_result9, 0
  br i1 %cond10, label %if_true5, label %if_next6

if_true5:                                         ; preds = %if_next
  br label %if_next6

if_next6:                                         ; preds = %if_true5, %if_next
  %sum11 = load i32, i32* %sum, align 4
  %i12 = load i32, i32* %i, align 4
  %add_result13 = add i32 %sum11, %i12
  store i32 %add_result13, i32* %sum, align 4
  br label %whileCond
}
