#include "ptrsx_unix.h"
#include <stdio.h>

int main() {
  // int pid = 26149;

  // init
  PointerSearcherX *ptr = ptrsx_init();
  int ret = 0;

  // create a pointer map in file
  // ret = create_pointer_map_file(ptr, pid, "./1.map");
  // if (ret != 0) {
  //   const char *error = get_last_error();
  //   printf("%s\n", error);
  //   return 0;
  // }

  // create a pointer map in memory
  // ret = create_pointer_map(ptr, pid);
  // if (ret != 0) {
  //   const char *error = get_last_error();
  //   printf("%s\n", error);
  //   return 0;
  // }

  // load pointer file
  ret = load_pointer_map_file(ptr, "1.dump");
  if (ret != 0) {
    const char *error = get_last_error();
    printf("%s\n", error);
    return 0;
  }

  // get available base address modules
  Modules modules = get_modules(ptr);
  for (int i = 0; i < modules.len; i++) {
    printf("[%zx %zx %s]\n", modules.data[i].start, modules.data[i].end,
           modules.data[i].name);
  }

  // select a base address module
  struct Module module = {modules.data[0].start, modules.data[0].end,
                          modules.data[0].name};

  struct Params params = {0x600002990020, 4, 3, 200, 200, "./hello.scandata"};

  // start scanner
  ret = scanner_pointer_chain_with_module(ptr, module, params);
  if (ret != 0) {
    const char *error = get_last_error();
    printf("%s\n", error);
    return 0;
  }

  clear_last_error();
  ptrsx_free(ptr);
  return 0;
}

// libptrsx.dylib