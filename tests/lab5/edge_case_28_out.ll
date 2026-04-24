; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  %i = alloca i32, align 4
  store i32 0, i32* %i, align 4
  %sum = alloca i32, align 4
  store i32 0, i32* %sum, align 4
  %count = alloca i32, align 4
  store i32 0, i32* %count, align 4
  br label %whileCond

whileCond:                                        ; preds = %if_next5, %mainEntry
  %i1 = load i32, i32* %i, align 4
  %cmp = icmp slt i32 %i1, 10
  br i1 %cmp, label %whileBody, label %whileNext

whileBody:                                        ; preds = %whileCond
  %i2 = load i32, i32* %i, align 4
  %mod_tmp = srem i32 %i2, 3
  %to_bool = icmp ne i32 %mod_tmp, 0
  %eq_tmp = icmp eq i1 %to_bool, false
  br i1 %eq_tmp, label %if_true, label %if_next

whileNext:                                        ; preds = %whileCond
  %sum15 = load i32, i32* %sum, align 4
  %count16 = load i32, i32* %count, align 4
  %add_tmp17 = add i32 %sum15, %count16
  ret i32 %add_tmp17

if_true:                                          ; preds = %whileBody
  %i3 = load i32, i32* %i, align 4
  %add_tmp = add i32 %i3, 1
  store i32 %add_tmp, i32* %i, align 4
  br label %if_next

if_next:                                          ; preds = %if_true, %whileBody
  %i6 = load i32, i32* %i, align 4
  %cmp7 = icmp sgt i32 %i6, 7
  br i1 %cmp7, label %if_true4, label %if_next5

if_true4:                                         ; preds = %if_next
  br label %if_next5

if_next5:                                         ; preds = %if_true4, %if_next
  %sum8 = load i32, i32* %sum, align 4
  %i9 = load i32, i32* %i, align 4
  %add_tmp10 = add i32 %sum8, %i9
  store i32 %add_tmp10, i32* %sum, align 4
  %count11 = load i32, i32* %count, align 4
  %add_tmp12 = add i32 %count11, 1
  store i32 %add_tmp12, i32* %count, align 4
  %i13 = load i32, i32* %i, align 4
  %add_tmp14 = add i32 %i13, 1
  store i32 %add_tmp14, i32* %i, align 4
  br label %whileCond
}
