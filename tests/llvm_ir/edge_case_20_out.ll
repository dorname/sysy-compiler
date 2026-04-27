; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  %i = alloca i32, align 4
  store i32 0, i32* %i, align 4
  %j = alloca i32, align 4
  store i32 0, i32* %j, align 4
  %count = alloca i32, align 4
  store i32 0, i32* %count, align 4
  br label %whileCond

whileCond:                                        ; preds = %if_next44, %mainEntry
  %i1 = load i32, i32* %i, align 4
  %cmp = icmp slt i32 %i1, 5
  %cmp_i32 = zext i1 %cmp to i32
  %cond_bool = icmp ne i32 %cmp_i32, 0
  %cond_result = zext i1 %cond_bool to i32
  %while_cond = icmp ne i32 %cond_result, 0
  br i1 %while_cond, label %whileBody, label %whileNext

whileBody:                                        ; preds = %whileCond
  store i32 0, i32* %j, align 4
  br label %whileCond2

whileNext:                                        ; preds = %whileCond
  %count55 = load i32, i32* %count, align 4
  ret i32 %count55

whileCond2:                                       ; preds = %if_next20, %whileBody
  %j5 = load i32, i32* %j, align 4
  %cmp6 = icmp slt i32 %j5, 3
  %cmp_i327 = zext i1 %cmp6 to i32
  %cond_bool8 = icmp ne i32 %cmp_i327, 0
  %cond_result9 = zext i1 %cond_bool8 to i32
  %while_cond10 = icmp ne i32 %cond_result9, 0
  br i1 %while_cond10, label %whileBody3, label %whileNext4

whileBody3:                                       ; preds = %whileCond2
  %i11 = load i32, i32* %i, align 4
  %to_bool = icmp ne i32 %i11, 0
  %bool_i32 = zext i1 %to_bool to i32
  %eq_tmp = icmp eq i32 %bool_i32, 1
  %eq_i32 = zext i1 %eq_tmp to i32
  %j12 = load i32, i32* %j, align 4
  %to_bool13 = icmp ne i32 %j12, 0
  %bool_i3214 = zext i1 %to_bool13 to i32
  %eq_tmp15 = icmp eq i32 %bool_i3214, 1
  %eq_i3216 = zext i1 %eq_tmp15 to i32
  %left_bool = icmp ne i32 %eq_i32, 0
  %right_bool = icmp ne i32 %eq_i3216, 0
  %and_bool = and i1 %left_bool, %right_bool
  %and_result = zext i1 %and_bool to i32
  %cond_bool17 = icmp ne i32 %and_result, 0
  %cond_result18 = zext i1 %cond_bool17 to i32
  %if_cond = icmp ne i32 %cond_result18, 0
  br i1 %if_cond, label %if_true, label %if_next

whileNext4:                                       ; preds = %whileCond2
  %i45 = load i32, i32* %i, align 4
  %to_bool46 = icmp ne i32 %i45, 0
  %bool_i3247 = zext i1 %to_bool46 to i32
  %eq_tmp48 = icmp eq i32 %bool_i3247, 1
  %eq_i3249 = zext i1 %eq_tmp48 to i32
  %cond_bool50 = icmp ne i32 %eq_i3249, 0
  %cond_result51 = zext i1 %cond_bool50 to i32
  %if_cond52 = icmp ne i32 %cond_result51, 0
  br i1 %if_cond52, label %if_true43, label %if_next44

if_true:                                          ; preds = %whileBody3
  br label %if_next

if_next:                                          ; preds = %if_true, %whileBody3
  %i21 = load i32, i32* %i, align 4
  %to_bool22 = icmp ne i32 %i21, 0
  %bool_i3223 = zext i1 %to_bool22 to i32
  %eq_tmp24 = icmp eq i32 %bool_i3223, 1
  %eq_i3225 = zext i1 %eq_tmp24 to i32
  %j26 = load i32, i32* %j, align 4
  %to_bool27 = icmp ne i32 %j26, 0
  %bool_i3228 = zext i1 %to_bool27 to i32
  %eq_tmp29 = icmp eq i32 %bool_i3228, 0
  %eq_i3230 = zext i1 %eq_tmp29 to i32
  %left_bool31 = icmp ne i32 %eq_i3225, 0
  %right_bool32 = icmp ne i32 %eq_i3230, 0
  %and_bool33 = and i1 %left_bool31, %right_bool32
  %and_result34 = zext i1 %and_bool33 to i32
  %cond_bool35 = icmp ne i32 %and_result34, 0
  %cond_result36 = zext i1 %cond_bool35 to i32
  %if_cond37 = icmp ne i32 %cond_result36, 0
  br i1 %if_cond37, label %if_true19, label %if_next20

if_true19:                                        ; preds = %if_next
  %j38 = load i32, i32* %j, align 4
  %add_tmp = add i32 %j38, 1
  store i32 %add_tmp, i32* %j, align 4
  br label %if_next20

if_next20:                                        ; preds = %if_true19, %if_next
  %count39 = load i32, i32* %count, align 4
  %add_tmp40 = add i32 %count39, 1
  store i32 %add_tmp40, i32* %count, align 4
  %j41 = load i32, i32* %j, align 4
  %add_tmp42 = add i32 %j41, 1
  store i32 %add_tmp42, i32* %j, align 4
  br label %whileCond2

if_true43:                                        ; preds = %whileNext4
  br label %if_next44

if_next44:                                        ; preds = %if_true43, %whileNext4
  %i53 = load i32, i32* %i, align 4
  %add_tmp54 = add i32 %i53, 1
  store i32 %add_tmp54, i32* %i, align 4
  br label %whileCond
}
