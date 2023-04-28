(module
    (func $if (param $cond i32) (result i32)
        (local.get $cond)
        (if (result i32)
            (then
                (i32.const 123)
            )
            (else
                (i32.const 456)
            )
        )
    )

    (func $nested_if (param $cond i32) (param $cond2 i32) (result i32)
        (local.get $cond)
        (if (result i32)
            (then
                (local.get $cond2)
                (if (result i32)
                    (then
                        (i32.const 789)
                    )
                    (else
                        (i32.const 0)
                    )
                )
            )
            (else
                (i32.const 456)
            )
        )
    )
)