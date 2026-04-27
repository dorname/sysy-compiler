; ModuleID = 'module'
source_filename = "module"

@count = global i32 0
@n = global i32 3

define void @hanoi(i32 %0, i32 %1, i32 %2, i32 %3) {
hanoiEntry:
  %n = alloca i32, align 4
  store i32 %0, i32* %n, align 4
  %source = alloca i32, align 4
  store i32 %1, i32* %source, align 4
  %target = alloca i32, align 4
  store i32 %2, i32* %target, align 4
  %auxiliary = alloca i32, align 4
  store i32 %3, i32* %auxiliary, align 4
  %tmp_ = load i32, i32* %n, align 4
  %cond_ = icmp eq i32 %tmp_, 1
  %cond_1 = zext i1 %cond_ to i32
  %cond_2 = icmp ne i32 %cond_1, 0
  br i1 %cond_2, label %if_true_, label %if_false_

if_true_:                                         ; preds = %hanoiEntry
  %tmp_3 = load i32, i32* @count, align 4
  %tmp_4 = add i32 %tmp_3, 1
  store i32 %tmp_4, i32* @count, align 4
  ret void

if_false_:                                        ; preds = %hanoiEntry
  br label %if_next_

if_next_:                                         ; preds = %if_false_, %if_true_
  %tmp_5 = load i32, i32* %n, align 4
  %tmp_6 = sub i32 %tmp_5, 1
  %tmp_7 = load i32, i32* %source, align 4
  %tmp_8 = load i32, i32* %auxiliary, align 4
  %tmp_9 = load i32, i32* %target, align 4
  call void @hanoi(i32 %tmp_6, i32 %tmp_7, i32 %tmp_8, i32 %tmp_9)
  %tmp_10 = load i32, i32* @count, align 4
  %tmp_11 = add i32 %tmp_10, 1
  store i32 %tmp_11, i32* @count, align 4
  %tmp_12 = load i32, i32* %n, align 4
  %tmp_13 = sub i32 %tmp_12, 1
  %tmp_14 = load i32, i32* %auxiliary, align 4
  %tmp_15 = load i32, i32* %target, align 4
  %tmp_16 = load i32, i32* %source, align 4
  call void @hanoi(i32 %tmp_13, i32 %tmp_14, i32 %tmp_15, i32 %tmp_16)
  ret void
}

define i32 @main() {
mainEntry:
  %tmp_ = load i32, i32* @n, align 4
  call void @hanoi(i32 %tmp_, i32 1, i32 3, i32 2)
  %tmp_1 = load i32, i32* @count, align 4
  ret i32 %tmp_1
}