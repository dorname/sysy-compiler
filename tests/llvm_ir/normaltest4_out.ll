; ModuleID = 'module'
source_filename = "module"

define i32 @defn() {
defnEntry:
  ret i32 4
}

define i32 @main() {
mainEntry:
  %defn = call i32 @defn()
  %a = alloca i32, align 4
  store i32 %defn, i32* %a, align 4
  %a1 = load i32, i32* %a, align 4
  ret i32 %a1
}
