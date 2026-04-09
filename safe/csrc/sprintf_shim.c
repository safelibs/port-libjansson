#include <stdarg.h>
#include <stdio.h>

#include "jansson.h"

void jsonp_error_vformat(json_error_t *error, int line, int column, size_t position, int code,
                         const char *text);

void jsonp_error_vset(json_error_t *error, int line, int column, size_t position,
                      enum json_error_code code, const char *msg, va_list ap) {
    char text[JSON_ERROR_TEXT_LENGTH - 1];

    if (!error)
        return;

    vsnprintf(text, sizeof(text), msg, ap);
    text[JSON_ERROR_TEXT_LENGTH - 2] = '\0';
    jsonp_error_vformat(error, line, column, position, code, text);
}

void jsonp_error_set(json_error_t *error, int line, int column, size_t position,
                     enum json_error_code code, const char *msg, ...) {
    va_list ap;

    va_start(ap, msg);
    jsonp_error_vset(error, line, column, position, code, msg, ap);
    va_end(ap);
}

json_t *json_sprintf(const char *fmt, ...) {
    (void)fmt;
    return NULL;
}

json_t *json_vsprintf(const char *fmt, va_list ap) {
    (void)fmt;
    (void)ap;
    return NULL;
}
