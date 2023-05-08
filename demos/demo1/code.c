int __attribute__((noinline)) super_safe_code(int x, int y) {
  if (x != 0) {
    return x / y; // oops
  } else {
    return x;
  }
}

int data[100];

void __attribute__((noinline)) even_safer_code(int value, int i) {
  data[i] = data[i] + value;
}

int __attribute__((noinline)) just_trust_me(int x, int i) {
  return x / data[i];
}

int main() {
  int z = super_safe_code(8, 4);
  even_safer_code(10, 1);
  data[0] = 1;
  z += just_trust_me(123, 0);
  return z;
}
