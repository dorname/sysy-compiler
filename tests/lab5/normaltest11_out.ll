; ModuleID = 'module'
source_filename = "module"

@gg = global i32 5

define i32 @main() {
mainEntry:
  %b = alloca i32, align 4
  store i32 12, i32* %b, align 4
  %b1 = load i32, i32* %b, align 4
  %gg = load i32, i32* @gg, align 4
  %mul_tmp = mul i32 %b1, %gg
  ret i32 %mul_tmp
}
