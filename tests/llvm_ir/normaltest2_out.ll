; ModuleID = 'module'
source_filename = "module"

define i32 @a(i32 %i) {
aEntry:
  %i2 = alloca i32, align 4
  store i32 %i, i32* %i2, align 4
  %i3 = load i32, i32* %i2, align 4
  ret i32 %i3
}

define i32 @b(i32 %i) {
bEntry:
  %i2 = alloca i32, align 4
  store i32 %i, i32* %i2, align 4
  %i3 = load i32, i32* %i2, align 4
  %i4 = load i32, i32* %i2, align 4
  %a = call i32 @a(i32 %i4)
  %mul_result = mul i32 %i3, %a
  ret i32 %mul_result
}

define i32 @c(i32 %i, i32 %k) {
cEntry:
  %i2 = alloca i32, align 4
  store i32 %i, i32* %i2, align 4
  %k4 = alloca i32, align 4
  store i32 %k, i32* %k4, align 4
  %i5 = load i32, i32* %i2, align 4
  %i6 = load i32, i32* %i2, align 4
  %b = call i32 @b(i32 %i6)
  %mul_result = mul i32 %i5, %b
  %k7 = load i32, i32* %k4, align 4
  %mul_result8 = mul i32 %mul_result, %k7
  ret i32 %mul_result8
}

define i32 @main() {
mainEntry:
  %c = call i32 @c(i32 2, i32 2)
  ret i32 %c
}
