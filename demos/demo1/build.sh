#!/bin/zsh

function build_c() {
    clang --target=wasm32 -O2 -nostdlib -Wl,--no-entry -Wl,--export-all -o code.wasm code.c
}

function disas_wasm() {
    wasm-objdump code.wasm -d -x
}

function each_func() {
    ../../target/debug/cli -i code.wasm -q
}

function only_main() {
    ../../target/debug/cli -i code.wasm -q --main 5
}

$1
