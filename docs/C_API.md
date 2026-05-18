# C API

The C ABI is defined in `include/memory_cpp.h` and implemented by `crates/memory-capi`.

Build:

```bash
cargo build -p memory-capi --release
```

On Windows this produces `memory_cpp.dll`; on Linux, `libmemory_cpp.so`; on macOS, `libmemory_cpp.dylib`.

Minimal usage:

```c
#include "memory_cpp.h"

memory_engine_t *engine = memory_engine_open("memory.db");
if (!engine) {
    fprintf(stderr, "%s\n", memory_last_error());
    return 1;
}

if (memory_engine_remember(engine, "Ship small APIs.", "fact", "project", 0.9) != 0) {
    fprintf(stderr, "%s\n", memory_last_error());
}

char *json = memory_engine_recall_json(engine, "What should we ship?", "project", 5);
if (json) {
    puts(json);
    memory_string_free(json);
}

memory_engine_free(engine);
```

## Ownership

All strings returned by `memory.cpp` must be released with `memory_string_free`.

All engine handles returned by `memory_engine_open` must be released with `memory_engine_free`.

`memory_last_error` returns a thread-local pointer owned by the library. Do not free it.
