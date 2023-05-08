export function checksum(): i32 {
  var v: i32 = 0x1505;
  var ptr: i32 = 0;
  var char: i32 = load<i8>(ptr);
  while (char != 0) {
    v = v * v + char;
    ptr = ptr + 1;
    char = load<i8>(ptr);
  }
  return v;
}
