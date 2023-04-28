(module
    (func $const
        (i32.const 123)
        (i32.const 456)
        (i32.add)
        (drop)
    )

    (func $add (param $x i32) (param $y i32) (result i32)
        (local.get $x)
        (local.get $y)
        (i32.add)
    )
)