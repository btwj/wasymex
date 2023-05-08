	.text
	.file	"test.c"
	.functype	super_safe_code (i32, i32) -> (i32)
	.functype	even_safer_code (i32, i32) -> ()
	.functype	getData () -> (i32)
	.functype	__original_main () -> (i32)
	.functype	main (i32, i32) -> (i32)
	.section	.text.super_safe_code,"",@
	.hidden	super_safe_code
	.globl	super_safe_code
	.type	super_safe_code,@function
super_safe_code:
	.functype	super_safe_code (i32, i32) -> (i32)
	block   	
	local.get	0
	br_if   	0
	i32.const	0
	return
.LBB0_2:
	end_block
	local.get	0
	local.get	1
	i32.div_s
	end_function
.Lfunc_end0:
	.size	super_safe_code, .Lfunc_end0-super_safe_code

	.section	.text.even_safer_code,"",@
	.hidden	even_safer_code
	.globl	even_safer_code
	.type	even_safer_code,@function
even_safer_code:
	.functype	even_safer_code (i32, i32) -> ()
	.local  	i32
	local.get	1
	i32.const	0
	local.get	1
	i32.const	0
	i32.gt_s
	i32.select
	local.set	2
	i32.const	data
	local.set	1
.LBB1_1:
	loop    	
	block   	
	local.get	2
	br_if   	0
	return
.LBB1_3:
	end_block
	local.get	1
	local.get	1
	i32.load	0
	local.get	0
	i32.add 
	i32.store	0
	local.get	1
	i32.const	4
	i32.add 
	local.set	1
	local.get	2
	i32.const	-1
	i32.add 
	local.set	2
	br      	0
.LBB1_4:
	end_loop
	end_function
.Lfunc_end1:
	.size	even_safer_code, .Lfunc_end1-even_safer_code

	.section	.text.getData,"",@
	.hidden	getData
	.globl	getData
	.type	getData,@function
getData:
	.functype	getData () -> (i32)
	i32.const	data
	end_function
.Lfunc_end2:
	.size	getData, .Lfunc_end2-getData

	.section	.text.__original_main,"",@
	.hidden	__original_main
	.globl	__original_main
	.type	__original_main,@function
__original_main:
	.functype	__original_main () -> (i32)
	i32.const	10
	i32.const	100000
	call	even_safer_code
	i32.const	0
	end_function
.Lfunc_end3:
	.size	__original_main, .Lfunc_end3-__original_main

	.section	.text.main,"",@
	.hidden	main
	.globl	main
	.type	main,@function
main:
	.functype	main (i32, i32) -> (i32)
	call	__original_main
	end_function
.Lfunc_end4:
	.size	main, .Lfunc_end4-main

	.hidden	data
	.type	data,@object
	.section	.bss.data,"",@
	.globl	data
	.p2align	4
data:
	.skip	40000
	.size	data, 40000

	.globl	__main_void
	.type	__main_void,@function
	.hidden	__main_void
.set __main_void, __original_main
	.ident	"Homebrew clang version 15.0.7"
	.no_dead_strip	__indirect_function_table
	.section	.custom_section.producers,"",@
	.int8	1
	.int8	12
	.ascii	"processed-by"
	.int8	1
	.int8	14
	.ascii	"Homebrew clang"
	.int8	6
	.ascii	"15.0.7"
	.section	.bss.data,"",@
