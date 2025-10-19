; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  %oct = alloca i32, align 4
  store i32 8, i32* %oct, align 4
  %hex = alloca i32, align 4
  store i32 26, i32* %hex, align 4
  %oct1 = load i32, i32* %oct, align 4
  %hex2 = load i32, i32* %hex, align 4
  %add_tmp = add i32 %oct1, %hex2
  ret i32 %add_tmp
}
