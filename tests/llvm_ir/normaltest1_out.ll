; ModuleID = 'module'
source_filename = "module"

define i32 @func() {
funcEntry:
  ret i32 1
}

define i32 @main() {
mainEntry:
  %func = call i32 @func()
  ret i32 %func
}
