(module
  (type (;0;) (func (param f64) (result f64)))
  (import "env" "mark" (func (;0;) (type 0) (param f64) (result f64)))
  (export "main" (func 1))
  (func (;1;) (type 0) (param f64) (result f64)
    (local f64)
    f64.const 0x1p+0 (;=1;)
    local.set 1
    f64.const 0x1.4p+1 (;=2.5;)
    local.set 1
    local.get 1
    f64.const 0x1p+0 (;=1;)
    f64.add
    return
  )
)
