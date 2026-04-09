#include <stdarg.h>

#include "jansson.h"

json_t *json_sprintf(const char *fmt, ...) {
    (void)fmt;
    return NULL;
}

json_t *json_vsprintf(const char *fmt, va_list ap) {
    (void)fmt;
    (void)ap;
    return NULL;
}
