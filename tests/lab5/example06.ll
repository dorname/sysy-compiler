; ModuleID = 'module'
source_filename = "module"

@a = global i32 0
@count = global i32 0

define i32 @main() {
mainEntry:
  br label %whileCond

whileCond:                                        ; preds = %if_next_, %mainEntry
  %tmp_ = load i32, i32* @a, align 4
  %cond_ = icmp sle i32 %tmp_, 0
  %cond_1 = zext i1 %cond_ to i32
  %cond_2 = icmp ne i32 %cond_1, 0
  br i1 %cond_2, label %whileBody, label %whileNext

whileBody:                                        ; preds = %whileCond
  %tmp_3 = load i32, i32* @a, align 4
  %tmp_4 = sub i32 %tmp_3, 1
  store i32 %tmp_4, i32* @a, align 4
  %tmp_5 = load i32, i32* @count, align 4
  %tmp_6 = add i32 %tmp_5, 1
  store i32 %tmp_6, i32* @count, align 4
  %tmp_7 = load i32, i32* @a, align 4
  %cond_8 = icmp slt i32 %tmp_7, -20
  %cond_9 = zext i1 %cond_8 to i32
  %cond_10 = icmp ne i32 %cond_9, 0
  br i1 %cond_10, label %if_true_, label %if_false_

whileNext:                                        ; preds = %if_true_, %whileCond
  %tmp_11 = load i32, i32* @count, align 4
  ret i32 %tmp_11

if_true_:                                         ; preds = %whileBody
  br label %whileNext

if_false_:                                        ; preds = %whileBody
  br label %if_next_

if_next_:                                         ; preds = %if_false_, %if_true_
  br label %whileCond
}