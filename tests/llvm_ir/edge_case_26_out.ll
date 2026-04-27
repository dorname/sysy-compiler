; ModuleID = 'module'
source_filename = "module"

define i32 @add(i32 %a, i32 %b) {
addEntry:
  %a2 = alloca i32, align 4
  store i32 %a, i32* %a2, align 4
  %b4 = alloca i32, align 4
  store i32 %b, i32* %b4, align 4
  %a5 = load i32, i32* %a2, align 4
  %b6 = load i32, i32* %b4, align 4
  %add_tmp = add i32 %a5, %b6
  ret i32 %add_tmp
}

define i32 @multiply(i32 %a, i32 %b) {
multiplyEntry:
  %a2 = alloca i32, align 4
  store i32 %a, i32* %a2, align 4
  %b4 = alloca i32, align 4
  store i32 %b, i32* %b4, align 4
  %a5 = load i32, i32* %a2, align 4
  %b6 = load i32, i32* %b4, align 4
  %mul_tmp = mul i32 %a5, %b6
  ret i32 %mul_tmp
}

define i32 @complex_calc(i32 %x, i32 %y, i32 %z) {
complex_calcEntry:
  %x2 = alloca i32, align 4
  store i32 %x, i32* %x2, align 4
  %y4 = alloca i32, align 4
  store i32 %y, i32* %y4, align 4
  %z6 = alloca i32, align 4
  store i32 %z, i32* %z6, align 4
  %x7 = load i32, i32* %x2, align 4
  %y8 = load i32, i32* %y4, align 4
  %add = call i32 @add(i32 %x7, i32 %y8)
  %temp1 = alloca i32, align 4
  store i32 %add, i32* %temp1, align 4
  %temp19 = load i32, i32* %temp1, align 4
  %z10 = load i32, i32* %z6, align 4
  %multiply = call i32 @multiply(i32 %temp19, i32 %z10)
  %temp2 = alloca i32, align 4
  store i32 %multiply, i32* %temp2, align 4
  %temp211 = load i32, i32* %temp2, align 4
  %x12 = load i32, i32* %x2, align 4
  %add13 = call i32 @add(i32 %temp211, i32 %x12)
  ret i32 %add13
}

define i32 @main() {
mainEntry:
  %complex_calc = call i32 @complex_calc(i32 2, i32 3, i32 4)
  ret i32 %complex_calc
}
