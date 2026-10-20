(module
  (type (;0;) (func (param f64) (result f64)))
  (import "env" "mark" (func (;0;) (type 0) (param f64) (result f64)))
  (export "main" (func 1))
  (func (;1;) (type 0) (param f64) (result f64)
    (local i32)
    i32.const 0
    local.set 1
    local.get 1
    f64.convert_i32_s
    f64.neg
    return
  )
)
