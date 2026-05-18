#include <stdio.h>
#include "memory_cpp.h"

int main(void) {
    memory_engine_t *engine = memory_engine_open(".memory.cpp/c-example.db");
    if (!engine) {
        fprintf(stderr, "open failed: %s\n", memory_last_error());
        return 1;
    }

    if (memory_engine_remember(
            engine,
            "C and C++ hosts can embed memory.cpp through a tiny ABI.",
            "fact",
            "example",
            0.9f
        ) != 0) {
        fprintf(stderr, "remember failed: %s\n", memory_last_error());
        memory_engine_free(engine);
        return 1;
    }

    char *json = memory_engine_recall_json(
        engine,
        "How can C++ use memory.cpp?",
        "example",
        5
    );

    if (!json) {
        fprintf(stderr, "recall failed: %s\n", memory_last_error());
        memory_engine_free(engine);
        return 1;
    }

    puts(json);
    memory_string_free(json);
    memory_engine_free(engine);
    return 0;
}
