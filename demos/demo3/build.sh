#!/bin/zsh

function run() {
    deno run --allow-read challenge.ts
}

function build() {
    asc --initialMemory 1 crack.ts > crack.wat
    wat2wasm --debug-names --enable-multi-memory crack.wat
}

function solve() {
    ../../target/debug/crack -i crack.wasm -q --main 0 --checksum -485194241
}

$1
