#!/bin/zsh

function build_zig() {
    zig build-lib div.zig -target wasm32-freestanding -dynamic -rdynamic -O ReleaseFast
}

function disas_wasm() {
    wasm-objdump div.wasm -d -x
}

function each_func() {
    ../../target/debug/cli -i div.wasm -q
}

$1
