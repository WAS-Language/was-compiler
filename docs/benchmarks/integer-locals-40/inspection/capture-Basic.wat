(module
  (type (;0;) (func (param f64) (result f64)))
  (type (;1;) (func (param i32) (result i32)))
  (type (;2;) (func (param i32 f64) (result i32)))
  (type (;3;) (func (param i32) (result f64)))
  (type (;4;) (func (param i32 i32) (result i32)))
  (import "env" "mark" (func (;0;) (type 0) (param f64) (result f64)))
  (memory (;0;) 1 65535)
  (global (;0;) (mut i32) i32.const 8)
  (export "main" (func 1))
  (export "memory" (memory 0))
  (export "__alloc" (func 2))
  (func (;1;) (type 0) (param f64) (result f64)
    (local f64 i32 i32)
    i32.const 120
    f64.convert_i32_s
    local.set 1
    i32.const 8
    call 2
    local.set 2
    local.get 2
    local.get 1
    f64.store
    i32.const 8
    call 2
    local.set 3
    local.get 3
    local.get 2
    i32.store
    local.get 2
    i32.const 121
    f64.convert_i32_s
    f64.store
    local.get 3
    call 9
    return
  )
  (func (;2;) (type 1) (param i32) (result i32)
    (local i64 i64 i64 i64)
    global.get 0
    i64.extend_i32_u
    local.tee 1
    i64.const 8
    i64.lt_u
    local.get 1
    i64.const 7
    i64.and
    i64.const 0
    i64.ne
    i32.or
    if ;; label = @1
      unreachable
    end
    local.get 0
    i64.extend_i32_u
    i64.const 7
    i64.add
    i64.const -8
    i64.and
    local.tee 2
    i64.eqz
    if ;; label = @1
      i64.const 8
      local.set 2
    end
    local.get 1
    local.get 2
    i64.add
    local.tee 3
    i64.const 4294901760
    i64.gt_u
    if ;; label = @1
      unreachable
    end
    local.get 3
    i64.const 65535
    i64.add
    i64.const 16
    i64.shr_u
    local.tee 4
    memory.size
    i64.extend_i32_u
    i64.gt_u
    if ;; label = @1
      local.get 4
      memory.size
      i64.extend_i32_u
      i64.sub
      i32.wrap_i64
      memory.grow
      i32.const -1
      i32.eq
      if ;; label = @2
        unreachable
      end
    end
    local.get 3
    i32.wrap_i64
    global.set 0
    local.get 1
    i32.wrap_i64
  )
  (func (;3;) (type 1) (param i32) (result i32)
    (local i64)
    local.get 0
    i32.const 8
    i32.lt_u
    local.get 0
    i32.const 7
    i32.and
    i32.const 0
    i32.ne
    i32.or
    if ;; label = @1
      unreachable
    end
    memory.size
    i64.extend_i32_u
    i64.const 16
    i64.shl
    local.set 1
    local.get 0
    i64.extend_i32_u
    i64.const 8
    i64.add
    local.get 1
    i64.gt_u
    if ;; label = @1
      unreachable
    end
    local.get 0
    i32.load
    i32.const 8
    i32.lt_u
    if ;; label = @1
      unreachable
    end
    local.get 0
    i32.load
    i64.extend_i32_u
    local.get 0
    i32.load offset=4
    i64.extend_i32_u
    i64.const 1
    i64.mul
    i64.add
    local.get 1
    i64.gt_u
    if ;; label = @1
      unreachable
    end
    local.get 0
  )
  (func (;4;) (type 1) (param i32) (result i32)
    (local i64)
    local.get 0
    i32.const 8
    i32.lt_u
    local.get 0
    i32.const 7
    i32.and
    i32.const 0
    i32.ne
    i32.or
    if ;; label = @1
      unreachable
    end
    memory.size
    i64.extend_i32_u
    i64.const 16
    i64.shl
    local.set 1
    local.get 0
    i64.extend_i32_u
    i64.const 8
    i64.add
    local.get 1
    i64.gt_u
    if ;; label = @1
      unreachable
    end
    local.get 0
    i32.load
    i32.const 8
    i32.lt_u
    if ;; label = @1
      unreachable
    end
    local.get 0
    i32.load
    i32.const 7
    i32.and
    i32.const 0
    i32.ne
    if ;; label = @1
      unreachable
    end
    local.get 0
    i32.load
    i64.extend_i32_u
    local.get 0
    i32.load offset=4
    i64.extend_i32_u
    i64.const 8
    i64.mul
    i64.add
    local.get 1
    i64.gt_u
    if ;; label = @1
      unreachable
    end
    local.get 0
  )
  (func (;5;) (type 2) (param i32 f64) (result i32)
    local.get 0
    call 4
    local.set 0
    local.get 1
    f64.const 0x0p+0 (;=0;)
    f64.ge
    local.get 1
    local.get 0
    i32.load offset=4
    f64.convert_i32_u
    f64.lt
    i32.and
    local.get 1
    local.get 1
    f64.trunc
    f64.eq
    i32.and
    i32.eqz
    if ;; label = @1
      unreachable
    end
    local.get 0
    i32.load
    i64.extend_i32_u
    local.get 1
    i64.trunc_f64_u
    i64.const 8
    i64.mul
    i64.add
    i32.wrap_i64
  )
  (func (;6;) (type 3) (param i32) (result f64)
    local.get 0
    call 4
    i32.load offset=4
    f64.convert_i32_u
  )
  (func (;7;) (type 4) (param i32 i32) (result i32)
    (local i32 i32 i32 i32)
    local.get 0
    call 3
    local.set 0
    local.get 1
    call 3
    local.set 1
    local.get 0
    i32.load offset=4
    local.tee 4
    local.get 1
    i32.load offset=4
    i32.ne
    if ;; label = @1
      i32.const 0
      return
    end
    local.get 0
    i32.load
    local.set 2
    local.get 1
    i32.load
    local.set 3
    block ;; label = @1
      loop ;; label = @2
        local.get 5
        local.get 4
        i32.ge_u
        br_if 1 (;@1;)
        local.get 2
        local.get 5
        i32.add
        i32.load8_u
        local.get 3
        local.get 5
        i32.add
        i32.load8_u
        i32.ne
        if ;; label = @3
          i32.const 0
          return
        end
        local.get 5
        i32.const 1
        i32.add
        local.set 5
        br 0 (;@2;)
      end
    end
    i32.const 1
  )
  (func (;8;) (type 4) (param i32 i32) (result i32)
    (local i32 i32 i32 i32)
    local.get 0
    call 4
    local.set 0
    local.get 1
    call 4
    local.set 1
    local.get 0
    i32.load offset=4
    local.tee 4
    local.get 1
    i32.load offset=4
    i32.ne
    if ;; label = @1
      i32.const 0
      return
    end
    local.get 0
    i32.load
    local.set 2
    local.get 1
    i32.load
    local.set 3
    block ;; label = @1
      loop ;; label = @2
        local.get 5
        local.get 4
        i32.ge_u
        br_if 1 (;@1;)
        local.get 2
        local.get 5
        i32.const 8
        i32.mul
        i32.add
        f64.load
        local.get 3
        local.get 5
        i32.const 8
        i32.mul
        i32.add
        f64.load
        f64.ne
        if ;; label = @3
          i32.const 0
          return
        end
        local.get 5
        i32.const 1
        i32.add
        local.set 5
        br 0 (;@2;)
      end
    end
    i32.const 1
  )
  (func (;9;) (type 3) (param i32) (result f64)
    local.get 0
    i32.load
    f64.load
    i32.const 23
    f64.convert_i32_s
    f64.add
  )
  (data (;0;) (i32.const 0) "\00\00\00\00\00\00\00\00")
)
