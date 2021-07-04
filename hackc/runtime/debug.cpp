#include <stdio.h>
#include <stdlib.h>

extern "C" long int hack_sys_init(long int**);

int main(int argc, char* argv[]) {
  long int* ram = NULL;

  long int return_code = hack_sys_init(&ram);
  ram[0] = return_code;
  if (argc >= 2) {
    int i = atoi(argv[1]);
    int j = i + 1;
    if (argc >= 3) {
      j = atoi(argv[2]);
    }
    for (; i < j; i++) {
      printf("%i:%ld\n", i, ram[i]);
    }
  }

  exit(EXIT_SUCCESS);
}