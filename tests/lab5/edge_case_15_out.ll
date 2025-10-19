; ModuleID = 'module'
source_filename = "module"

@a = global i32 1
@b = global i32 2
@c = global i32 3

define i32 @main() {
mainEntry:
  %a = load i32, i32* @a, align 4
  %b = load i32, i32* @b, align 4
  %add_tmp = add i32 %a, %b
  %c = load i32, i32* @c, align 4
  %add_tmp1 = add i32 %add_tmp, %c
  ret i32 %add_tmp1
}
