(module
  (type (;0;) (func (param f64) (result f64)))
  (import "env" "mark" (func (;0;) (type 0) (param f64) (result f64)))
  (export "main" (func 1))
  (func (;1;) (type 0) (param f64) (result f64)
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.lt
    if (result f64) ;; label = @1
      f64.const 0x1p+0 (;=1;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.le
    if (result f64) ;; label = @1
      f64.const 0x1p+1 (;=2;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ep+7 (;=151;)
    f64.neg
    f64.gt
    if (result f64) ;; label = @1
      f64.const 0x1p+2 (;=4;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.ge
    if (result f64) ;; label = @1
      f64.const 0x1p+3 (;=8;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2cp+7 (;=150;)
    f64.neg
    f64.eq
    if (result f64) ;; label = @1
      f64.const 0x1p+4 (;=16;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.const 0x1.ep+6 (;=120;)
    f64.const 0x1.ep+4 (;=30;)
    f64.add
    f64.neg
    f64.const 0x1.2ap+7 (;=149;)
    f64.neg
    f64.ne
    if (result f64) ;; label = @1
      f64.const 0x1p+5 (;=32;)
    else
      f64.const 0x0p+0 (;=0;)
    end
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    f64.add
    return
  )
)
