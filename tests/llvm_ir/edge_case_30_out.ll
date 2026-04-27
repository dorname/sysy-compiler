; ModuleID = 'module'
source_filename = "module"

define i32 @main() {
mainEntry:
  %a = alloca i32, align 4
  store i32 2, i32* %a, align 4
  %b = alloca i32, align 4
  store i32 3, i32* %b, align 4
  %c = alloca i32, align 4
  store i32 4, i32* %c, align 4
  %d = alloca i32, align 4
  store i32 5, i32* %d, align 4
  %a1 = load i32, i32* %a, align 4
  %b2 = load i32, i32* %b, align 4
  %c3 = load i32, i32* %c, align 4
  %mul_tmp = mul i32 %b2, %c3
  %add_tmp = add i32 %a1, %mul_tmp
  %d4 = load i32, i32* %d, align 4
  %a5 = load i32, i32* %a, align 4
  %div_tmp = sdiv i32 %d4, %a5
  %sub_tmp = sub i32 %add_tmp, %div_tmp
  %a6 = load i32, i32* %a, align 4
  %b7 = load i32, i32* %b, align 4
  %add_tmp8 = add i32 %a6, %b7
  %c9 = load i32, i32* %c, align 4
  %d10 = load i32, i32* %d, align 4
  %sub_tmp11 = sub i32 %c9, %d10
  %mul_tmp12 = mul i32 %add_tmp8, %sub_tmp11
  %add_tmp13 = add i32 %sub_tmp, %mul_tmp12
  %a14 = load i32, i32* %a, align 4
  %b15 = load i32, i32* %b, align 4
  %mod_tmp = srem i32 %a14, %b15
  %add_tmp16 = add i32 %add_tmp13, %mod_tmp
  %c17 = load i32, i32* %c, align 4
  %d18 = load i32, i32* %d, align 4
  %div_tmp19 = sdiv i32 %c17, %d18
  %add_tmp20 = add i32 %add_tmp16, %div_tmp19
  ret i32 %add_tmp20
}
