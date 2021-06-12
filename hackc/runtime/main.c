#include <stdio.h>
#include <stdlib.h>

extern void hack_sys_init(long int**);

int main() {
  printf("Hello world!\n");
  long int* ram = NULL;

  hack_sys_init(&ram);
  printf("output is (at %p):\n", ram);
  for (int i = 0; i < 16; i++) {
    printf("  [%2i] = %x\n", i, (short)ram[i]);
  }
  exit(EXIT_SUCCESS);
}