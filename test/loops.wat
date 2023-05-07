(module
    (func $loopconc (result i32) (local $sum i32) (local $count i32)
        (local.set $count (i32.const 3))
        (loop $l
            (local.set $sum (i32.add (local.get $sum) (local.get $count)))
            (local.set $count (i32.sub (local.get $count) (i32.const 1)))
            (br_if $l (i32.gt_s (local.get $count) (i32.const 0)))
        )
        (local.get $sum)
    )

    (func $loopsym (param $iters i32) (result i32) (local $sum i32) (local $count i32)
        (local.set $count (local.get $iters))
        (loop $l
            (local.set $sum (i32.add (local.get $sum) (local.get $count)))
            (local.set $count (i32.sub (local.get $count) (i32.const 1)))
            (br_if $l (i32.gt_s (local.get $count) (i32.const 0)))
        )
        (local.get $sum)
    )
)