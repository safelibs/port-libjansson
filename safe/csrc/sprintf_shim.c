#include <stdarg.h>
#include <stdio.h>

#include "jansson.h"

void *jsonp_malloc(size_t size);
void jsonp_free(void *ptr);
void jsonp_error_vformat(json_error_t *error, int line, int column, size_t position, int code,
                         const char *text);
json_t *jsonp_sprintf_string_own(char *value, size_t len);

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

json_t *json_vsprintf(const char *fmt, va_list ap) {
    json_t *json = NULL;
    int length;
    char *buf;
    va_list aq;

    va_copy(aq, ap);

    length = vsnprintf(NULL, 0, fmt, ap);
    if (length < 0)
        goto out;

    buf = jsonp_malloc((size_t)length + 1);
    if (!buf)
        goto out;

    vsnprintf(buf, (size_t)length + 1, fmt, aq);
    json = jsonp_sprintf_string_own(buf, (size_t)length);

out:
    va_end(aq);
    return json;
}

json_t *json_sprintf(const char *fmt, ...) {
    json_t *json;
    va_list ap;

    va_start(ap, fmt);
    json = json_vsprintf(fmt, ap);
    va_end(ap);

    return json;
}
