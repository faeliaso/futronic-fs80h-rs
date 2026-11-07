#include "futronic_fs80h.h"
#include <stdio.h>

int main(void) {
  if (fs80h_abi_version() != FS80H_ABI_VERSION) return 1;
  size_t count = 0;
  fs80h_status status = fs80h_list_devices(NULL, 0, &count);
  if (status != FS80H_OK) {
    fprintf(stderr, "enumeration failed: %s\n", fs80h_last_error_message());
    return 2;
  }
  printf("ABI %u; %zu scanner(s) found\n", fs80h_abi_version(), count);
  return 0;
}
