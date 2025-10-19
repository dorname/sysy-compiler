; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  %a = alloca i32, align 4
  store i32 -5, i32* %a, align 4
  %b = alloca i32, align 4
  store i32 3, i32* %b, align 4
  %a1 = load i32, i32* %a, align 4
  %b2 = load i32, i32* %b, align 4
  %add_tmp = add i32 %a1, %b2
  ret i32 %add_tmp
}
