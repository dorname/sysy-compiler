; ModuleID = 'module'
source_filename = "module"

define i32 @max(i32 %a, i32 %b) {
maxEntry:
  %a2 = alloca i32, align 4
  store i32 %a, i32* %a2, align 4
  %b4 = alloca i32, align 4
  store i32 %b, i32* %b4, align 4
  %a5 = load i32, i32* %a2, align 4
  %b6 = load i32, i32* %b4, align 4
  %cmp = icmp sgt i32 %a5, %b6
  %cmp_i32 = zext i1 %cmp to i32
  %cond_bool = icmp ne i32 %cmp_i32, 0
  %cond_result = zext i1 %cond_bool to i32
  %if_cond = icmp ne i32 %cond_result, 0
  br i1 %if_cond, label %if_true, label %if_false

if_true:                                          ; preds = %maxEntry
  %a7 = load i32, i32* %a2, align 4
  ret i32 %a7

if_next:                                          ; No predecessors!
  ret i32 0

if_false:                                         ; preds = %maxEntry
  %b8 = load i32, i32* %b4, align 4
  ret i32 %b8
}

define i32 @min(i32 %a, i32 %b) {
minEntry:
  %a2 = alloca i32, align 4
  store i32 %a, i32* %a2, align 4
  %b4 = alloca i32, align 4
  store i32 %b, i32* %b4, align 4
  %a5 = load i32, i32* %a2, align 4
  %b6 = load i32, i32* %b4, align 4
  %cmp = icmp slt i32 %a5, %b6
  %cmp_i32 = zext i1 %cmp to i32
  %cond_bool = icmp ne i32 %cmp_i32, 0
  %cond_result = zext i1 %cond_bool to i32
  %if_cond = icmp ne i32 %cond_result, 0
  br i1 %if_cond, label %if_true, label %if_false

if_true:                                          ; preds = %minEntry
  %a7 = load i32, i32* %a2, align 4
  ret i32 %a7

if_next:                                          ; No predecessors!
  ret i32 0

if_false:                                         ; preds = %minEntry
  %b8 = load i32, i32* %b4, align 4
  ret i32 %b8
}

define i32 @main() {
mainEntry:
  %x = alloca i32, align 4
  store i32 10, i32* %x, align 4
  %y = alloca i32, align 4
  store i32 20, i32* %y, align 4
  %z = alloca i32, align 4
  store i32 15, i32* %z, align 4
  %x1 = load i32, i32* %x, align 4
  %y2 = load i32, i32* %y, align 4
  %min = call i32 @min(i32 %x1, i32 %y2)
  %z3 = load i32, i32* %z, align 4
  %max = call i32 @max(i32 %min, i32 %z3)
  ret i32 %max
}
