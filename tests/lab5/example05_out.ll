; ModuleID = 'module'
source_filename = "module"

define i32 @f(i32 %i) {
fEntry:
  %i2 = alloca i32, align 4
  store i32 %i, i32* %i2, align 4
  %i3 = load i32, i32* %i2, align 4
  ret i32 %i3
}

define i32 @main() {
mainEntry:
  %a = alloca i32, align 4
  store i32 1, i32* %a, align 4
  %a1 = load i32, i32* %a, align 4
  %f = call i32 @f(i32 %a1)
  ret i32 %f
}
