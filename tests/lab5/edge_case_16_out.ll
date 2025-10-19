; ModuleID = 'module'
source_filename = "module"

define i32 @f1(i32 %x) {
f1Entry:
  %x2 = alloca i32, align 4
  store i32 %x, i32* %x2, align 4
  %x3 = load i32, i32* %x2, align 4
  %add_tmp = add i32 %x3, 1
  ret i32 %add_tmp
}

define i32 @f2(i32 %x) {
f2Entry:
  %x2 = alloca i32, align 4
  store i32 %x, i32* %x2, align 4
  %x3 = load i32, i32* %x2, align 4
  %f1 = call i32 @f1(i32 %x3)
  %mul_tmp = mul i32 %f1, 2
  ret i32 %mul_tmp
}

define i32 @f3(i32 %x) {
f3Entry:
  %x2 = alloca i32, align 4
  store i32 %x, i32* %x2, align 4
  %x3 = load i32, i32* %x2, align 4
  %f2 = call i32 @f2(i32 %x3)
  %x4 = load i32, i32* %x2, align 4
  %f1 = call i32 @f1(i32 %x4)
  %sub_tmp = sub i32 %f2, %f1
  ret i32 %sub_tmp
}

define i32 @main() {
mainEntry:
  %f3 = call i32 @f3(i32 5)
  ret i32 %f3
}
