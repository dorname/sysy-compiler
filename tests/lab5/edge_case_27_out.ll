; ModuleID = 'module'
source_filename = "module"

define i32 @is_even(i32 %n) {
is_evenEntry:
  %n2 = alloca i32, align 4
  store i32 %n, i32* %n2, align 4
  ret i32 0
}

define i32 @is_positive(i32 %n) {
is_positiveEntry:
  %n2 = alloca i32, align 4
  store i32 %n, i32* %n2, align 4
  ret i32 0
}

define i32 @main() {
mainEntry:
  %a = alloca i32, align 4
  store i32 6, i32* %a, align 4
  %b = alloca i32, align 4
  store i32 -3, i32* %b, align 4
  %a1 = load i32, i32* %a, align 4
  %is_even = call i32 @is_even(i32 %a1)
  %to_bool = icmp ne i32 %is_even, 0
  %a2 = load i32, i32* %a, align 4
  %is_positive = call i32 @is_positive(i32 %a2)
  %to_bool3 = icmp ne i32 %is_positive, 0
  %and_bool = and i1 %to_bool, %to_bool3
  br i1 %and_bool, label %if_true, label %if_false

if_true:                                          ; preds = %mainEntry
  %b6 = load i32, i32* %b, align 4
  %is_even7 = call i32 @is_even(i32 %b6)
  %to_bool8 = icmp ne i32 %is_even7, 0
  %b9 = load i32, i32* %b, align 4
  %is_positive10 = call i32 @is_positive(i32 %b9)
  %to_bool11 = icmp ne i32 %is_positive10, 0
  %or_bool = or i1 %to_bool8, %to_bool11
  br i1 %or_bool, label %if_true4, label %if_false12

if_next:                                          ; preds = %if_next5
  ret i32 0

if_false:                                         ; preds = %mainEntry
  ret i32 3

if_true4:                                         ; preds = %if_true
  ret i32 1

if_next5:                                         ; No predecessors!
  br label %if_next

if_false12:                                       ; preds = %if_true
  ret i32 2
}
