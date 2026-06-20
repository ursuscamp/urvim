/* C syntax fixture */
// line comment
#include <stdio.h>
#define MAX_SIZE 16

int main(void) {
  char ch = '\n';
  unsigned long value = 0xffu;
  printf("value=%u\n", 42);
  fprintf(stderr, "error=%d: %s\n", 7, "tail");
  printf("%s %s", "first", "second");
  compute(value);
  return 0;
}
