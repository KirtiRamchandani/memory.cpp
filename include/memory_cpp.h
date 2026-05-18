#ifndef MEMORY_CPP_H
#define MEMORY_CPP_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct memory_engine_t memory_engine_t;

memory_engine_t *memory_engine_open(const char *path);
void memory_engine_free(memory_engine_t *engine);

int32_t memory_engine_remember(
    memory_engine_t *engine,
    const char *content,
    const char *kind,
    const char *scope,
    float importance
);

char *memory_engine_recall_json(
    memory_engine_t *engine,
    const char *query,
    const char *scope,
    uintptr_t limit
);

char *memory_engine_context(
    memory_engine_t *engine,
    const char *query,
    const char *scope,
    uintptr_t limit,
    uintptr_t token_budget
);

int32_t memory_engine_delete(memory_engine_t *engine, const char *id);
char *memory_engine_stats_json(memory_engine_t *engine);

const char *memory_last_error(void);
void memory_string_free(char *value);

#ifdef __cplusplus
}
#endif

#endif
