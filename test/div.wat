(module
    (func $ifdiv (param $cond i32) (param $dividend i32) (result i32)
        (local.get $cond)
        (if (result i32)
            (then
                (i32.div_s (local.get $dividend) (i32.const 0))
            )
            (else
                (i32.div_s (local.get $dividend) (i32.const 1))
            )
        )
    )

    (func $unsafediv (param $divisor i32) (param $dividend i32) (result i32)
        (local.get $dividend)
        (local.get $divisor)
        (i32.div_s)
    )

    (func $safediv (param $divisor i32) (param $dividend i32) (result i32)
        (local.get $divisor)
        (i32.const 0)
        (i32.gt_s)
        (if (result i32)
            (then
                (local.get $dividend)
                (local.get $divisor)
                (i32.div_s)
            )
            (else
                (i32.const -1)
            )
        )
    )
)