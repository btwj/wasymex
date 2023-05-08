int super_safe_code(int x, int y) {
  if (x != 0) {
    return x / y;
  } else {
    return x;
  }
}

int data[10000];
void even_safer_code(int value, int len) {
  for (int i = 0; i < len; i++) {
    data[i] = data[i] + value;
  }
}

int *getData() { return &data[0]; }

int main() {
  int z = super_safe_code(8, 4);
  even_safer_code(10, 100000);
  return 0;
}
