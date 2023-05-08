const crackMe = new WebAssembly.Module(Deno.readFileSync("crack.wasm"));
const encoder = new TextEncoder();

while (true) {
  let input = prompt("password: ");
  let encodedInput = encoder.encode(input);

  const instance = new WebAssembly.Instance(
    crackMe,
  );
  const memory = instance.exports.memory;
  const bytes = new Uint8Array(memory.buffer);
  bytes.set(encodedInput, 0);
  bytes.set([0], encodedInput.length);
  if (instance.exports.crack() == -485194241) {
    console.log("you win");
    break;
  } else {
    console.log("you lose");
  }
}
