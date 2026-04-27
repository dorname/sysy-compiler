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
  %cmp_i32 = zext i1 %cmp to i32
  %cond = icmp ne i32 %cmp_i32, 0
  br i1 %cond, label %whileBody, label %whileNext

whileBody:                                        ; preds = %whileCond
  store i32 0, i32* %j, align 4
  br label %whileCond2

whileNext:                                        ; preds = %whileCond
  %sum14 = load i32, i32* %sum, align 4
  ret i32 %sum14

whileCond2:                                       ; preds = %whileBody3, %whileBody
  %j5 = load i32, i32* %j, align 4
  %cmp6 = icmp slt i32 %j5, 2
  %cmp_i327 = zext i1 %cmp6 to i32
  %cond8 = icmp ne i32 %cmp_i327, 0
  br i1 %cond8, label %whileBody3, label %whileNext4

whileBody3:                                       ; preds = %whileCond2
  %sum9 = load i32, i32* %sum, align 4
  %add_tmp = add i32 %sum9, 1
  store i32 %add_tmp, i32* %sum, align 4
  %j10 = load i32, i32* %j, align 4
  %add_tmp11 = add i32 %j10, 1
  store i32 %add_tmp11, i32* %j, align 4
  br label %whileCond2

whileNext4:                                       ; preds = %whileCond2
  %i12 = load i32, i32* %i, align 4
  %add_tmp13 = add i32 %i12, 1
  store i32 %add_tmp13, i32* %i, align 4
  br label %whileCond
}
