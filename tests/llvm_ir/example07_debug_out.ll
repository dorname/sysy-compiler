; ModuleID = 'module'
source_filename = "module"

@count = global i32 0
@n = global i32 3

define void @hanoi(i32 %n, i32 %source, i32 %target, i32 %auxiliary) {
hanoiEntry:
  %n2 = alloca i32, align 4
  store i32 %n, i32* %n2, align 4
  %source4 = alloca i32, align 4
  store i32 %source, i32* %source4, align 4
  %target6 = alloca i32, align 4
  store i32 %target, i32* %target6, align 4
  %auxiliary8 = alloca i32, align 4
  store i32 %auxiliary, i32* %auxiliary8, align 4
  %n9 = load i32, i32* %n2, align 4
  %to_bool = icmp ne i32 %n9, 0
  %eq_tmp = icmp eq i1 %to_bool, true
  br i1 %eq_tmp, label %if_true, label %if_next

if_true:                                          ; preds = %hanoiEntry
  %count = load i32, i32* @count, align 4
  %add_tmp = add i32 %count, 1
  store i32 %add_tmp, i32* @count, align 4
  ret void

if_next:                                          ; preds = %hanoiEntry
  %n10 = load i32, i32* %n2, align 4
  %sub_tmp = sub i32 %n10, 1
  %source11 = load i32, i32* %source4, align 4
  %auxiliary12 = load i32, i32* %auxiliary8, align 4
  %target13 = load i32, i32* %target6, align 4
  call void @hanoi(i32 %sub_tmp, i32 %source11, i32 %auxiliary12, i32 %target13)
  %count14 = load i32, i32* @count, align 4
  %add_tmp15 = add i32 %count14, 1
  store i32 %add_tmp15, i32* @count, align 4
  %n16 = load i32, i32* %n2, align 4
  %sub_tmp17 = sub i32 %n16, 1
  %auxiliary18 = load i32, i32* %auxiliary8, align 4
  %target19 = load i32, i32* %target6, align 4
  %source20 = load i32, i32* %source4, align 4
  call void @hanoi(i32 %sub_tmp17, i32 %auxiliary18, i32 %target19, i32 %source20)
  ret void
}

define i32 @main() {
mainEntry:
  %n = load i32, i32* @n, align 4
  call void @hanoi(i32 %n, i32 1, i32 3, i32 2)
  %count = load i32, i32* @count, align 4
  ret i32 %count
}
