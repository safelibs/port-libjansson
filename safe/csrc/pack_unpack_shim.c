#include <stdarg.h>

#include "jansson.h"

json_t *json_pack(const char *fmt, ...) {
    (void)fmt;
    return NULL;
}

json_t *json_pack_ex(json_error_t *error, size_t flags, const char *fmt, ...) {
    (void)error;
    (void)flags;
    (void)fmt;
    return NULL;
}

json_t *json_vpack_ex(json_error_t *error, size_t flags, const char *fmt, va_list ap) {
    (void)error;
    (void)flags;
    (void)fmt;
    (void)ap;
    return NULL;
}

int json_unpack(json_t *root, const char *fmt, ...) {
    (void)root;
    (void)fmt;
    return -1;
}

int json_unpack_ex(json_t *root, json_error_t *error, size_t flags, const char *fmt, ...) {
    (void)root;
    (void)error;
    (void)flags;
    (void)fmt;
    return -1;
}

int json_vunpack_ex(json_t *root, json_error_t *error, size_t flags, const char *fmt,
                    va_list ap) {
    (void)root;
    (void)error;
    (void)flags;
    (void)fmt;
    (void)ap;
    return -1;
}
