; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  %i = alloca i32, align 4
  store i32 0, i32* %i, align 4
  %j = alloca i32, align 4
  store i32 0, i32* %j, align 4
  %sum = alloca i32, align 4
  store i32 0, i32* %sum, align 4
  br label %whileCond

whileCond:                                        ; preds = %whileNext4, %mainEntry
  %i1 = load i32, i32* %i, align 4
  %cmp = icmp slt i32 %i1, 3
  br i1 %cmp, label %whileBody, label %whileNext

whileBody:                                        ; preds = %whileCond
  store i32 0, i32* %j, align 4
  br label %whileCond2

whileNext:                                        ; preds = %whileCond
  %sum20 = load i32, i32* %sum, align 4
  ret i32 %sum20

whileCond2:                                       ; preds = %if_next, %whileBody
  %j5 = load i32, i32* %j, align 4
  %cmp6 = icmp slt i32 %j5, 2
  br i1 %cmp6, label %whileBody3, label %whileNext4

whileBody3:                                       ; preds = %whileCond2
  %i7 = load i32, i32* %i, align 4
  %j8 = load i32, i32* %j, align 4
  %add_tmp = add i32 %i7, %j8
  %cmp9 = icmp sgt i32 %add_tmp, 2
  br i1 %cmp9, label %if_true, label %if_false

whileNext4:                                       ; preds = %whileCond2
  %i18 = load i32, i32* %i, align 4
  %add_tmp19 = add i32 %i18, 1
  store i32 %add_tmp19, i32* %i, align 4
  br label %whileCond

if_true:                                          ; preds = %whileBody3
  %sum10 = load i32, i32* %sum, align 4
  %i11 = load i32, i32* %i, align 4
  %j12 = load i32, i32* %j, align 4
  %mul_tmp = mul i32 %i11, %j12
  %add_tmp13 = add i32 %sum10, %mul_tmp
  store i32 %add_tmp13, i32* %sum, align 4
  br label %if_next

if_next:                                          ; preds = %if_false, %if_true
  %j16 = load i32, i32* %j, align 4
  %add_tmp17 = add i32 %j16, 1
  store i32 %add_tmp17, i32* %j, align 4
  br label %whileCond2

if_false:                                         ; preds = %whileBody3
  %sum14 = load i32, i32* %sum, align 4
  %add_tmp15 = add i32 %sum14, 1
  store i32 %add_tmp15, i32* %sum, align 4
  br label %if_next
}
