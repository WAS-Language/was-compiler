(module
  (type (;0;) (func (param f64) (result f64)))
  (import "env" "mark" (func (;0;) (type 0) (param f64) (result f64)))
  (export "main" (func 1))
  (func (;1;) (type 0) (param f64) (result f64)
    f64.const 0x0p+0 (;=0;)
    f64.const 0x1.fffffffcp+30 (;=2147483647;)
    f64.sub
    f64.const 0x1p+0 (;=1;)
    f64.sub
    f64.neg
    return
  )
)
