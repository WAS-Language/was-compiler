(module
  (type (;0;) (func (param f64) (result f64)))
  (import "env" "mark" (func (;0;) (type 0) (param f64) (result f64)))
  (export "main" (func 1))
  (func (;1;) (type 0) (param f64) (result f64)
    i32.const 0
    i32.const 120
    i32.const 30
    i32.add
    i32.sub
    i32.const 0
    i32.const 149
    i32.sub
    i32.lt_s
    if (result f64) ;; label = @1
      i32.const 1
      f64.convert_i32_s
    else
      i32.const 0
      f64.convert_i32_s
    end
    i32.const 0
    i32.const 120
    i32.const 30
    i32.add
    i32.sub
    i32.const 0
    i32.const 150
    i32.sub
    i32.le_s
    if (result f64) ;; label = @1
      i32.const 2
      f64.convert_i32_s
    else
      i32.const 0
      f64.convert_i32_s
    end
    i32.const 0
    i32.const 120
    i32.const 30
    i32.add
    i32.sub
    i32.const 0
    i32.const 151
    i32.sub
    i32.gt_s
    if (result f64) ;; label = @1
      i32.const 4
      f64.convert_i32_s
    else
      i32.const 0
      f64.convert_i32_s
    end
    f64.add
    f64.add
    i32.const 0
    i32.const 120
    i32.const 30
    i32.add
    i32.sub
    i32.const 0
    i32.const 150
    i32.sub
    i32.ge_s
    if (result f64) ;; label = @1
      i32.const 8
      f64.convert_i32_s
    else
      i32.const 0
      f64.convert_i32_s
    end
    i32.const 0
    i32.const 120
    i32.const 30
    i32.add
    i32.sub
    i32.const 0
    i32.const 150
    i32.sub
    i32.eq
    if (result f64) ;; label = @1
      i32.const 16
      f64.convert_i32_s
    else
      i32.const 0
      f64.convert_i32_s
    end
    i32.const 0
    i32.const 120
    i32.const 30
    i32.add
    i32.sub
    i32.const 0
    i32.const 149
    i32.sub
    i32.ne
    if (result f64) ;; label = @1
      i32.const 32
      f64.convert_i32_s
    else
      i32.const 0
      f64.convert_i32_s
    end
    f64.add
    f64.add
    f64.add
    return
  )
)
