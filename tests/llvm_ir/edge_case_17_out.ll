; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  %a = alloca i32, align 4
  store i32 10, i32* %a, align 4
  %b = alloca i32, align 4
  store i32 5, i32* %b, align 4
  %c = alloca i32, align 4
  store i32 3, i32* %c, align 4
  %a1 = load i32, i32* %a, align 4
  %b2 = load i32, i32* %b, align 4
  %cmp = icmp sgt i32 %a1, %b2
  br i1 %cmp, label %if_true, label %if_false

if_true:                                          ; preds = %mainEntry
  %b5 = load i32, i32* %b, align 4
  %c6 = load i32, i32* %c, align 4
  %cmp7 = icmp sgt i32 %b5, %c6
  br i1 %cmp7, label %if_true3, label %if_false8

if_next:                                          ; preds = %if_next16, %if_next4
  ret i32 0

if_false:                                         ; preds = %mainEntry
  %b17 = load i32, i32* %b, align 4
  %c18 = load i32, i32* %c, align 4
  %cmp19 = icmp sgt i32 %b17, %c18
  br i1 %cmp19, label %if_true15, label %if_false20

if_true3:                                         ; preds = %if_true
  %a11 = load i32, i32* %a, align 4
  %c12 = load i32, i32* %c, align 4
  %cmp13 = icmp sgt i32 %a11, %c12
  br i1 %cmp13, label %if_true9, label %if_false14

if_next4:                                         ; preds = %if_next10
  br label %if_next

if_false8:                                        ; preds = %if_true
  ret i32 3

if_true9:                                         ; preds = %if_true3
  ret i32 1

if_next10:                                        ; No predecessors!
  br label %if_next4

if_false14:                                       ; preds = %if_true3
  ret i32 2

if_true15:                                        ; preds = %if_false
  ret i32 4

if_next16:                                        ; No predecessors!
  br label %if_next

if_false20:                                       ; preds = %if_false
  ret i32 5
}
