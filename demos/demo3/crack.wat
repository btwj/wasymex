(module
 (type $none_=>_i32 (func (result i32)))
 (global $~lib/memory/__data_end i32 (i32.const 8))
 (global $~lib/memory/__stack_pointer (mut i32) (i32.const 32776))
 (global $~lib/memory/__heap_base i32 (i32.const 32776))
 (memory $0 1)
 (table $0 1 1 funcref)
 (elem $0 (i32.const 1))
 (export "crack" (func $crack/crack))
 (export "memory" (memory $0))
 (func $crack/crack (result i32)
  (local $0 i32)
  (local $1 i32)
  (local $2 i32)
  i32.const 291
  local.set $0
  i32.const 0
  local.set $1
  local.get $1
  i32.load8_s $0
  local.set $2
  loop $while-continue|0
   local.get $2
   i32.const 0
   i32.ne
   if
    local.get $0
    local.get $0
    i32.mul
    local.get $2
    i32.add
    local.set $0
    local.get $1
    i32.const 1
    i32.add
    local.set $1
    local.get $1
    i32.load8_s $0
    local.set $2
    br $while-continue|0
   end
  end
  local.get $0
  return
 )
)
