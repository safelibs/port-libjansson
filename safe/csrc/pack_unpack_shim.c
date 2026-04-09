#include <stdarg.h>
#include <stdint.h>
#include <string.h>

#include "jansson.h"

typedef struct {
    int line;
    int column;
    size_t pos;
    char token;
} token_t;

typedef struct {
    const char *fmt;
    token_t prev_token;
    token_t token;
    token_t next_token;
    int line;
    int column;
    size_t pos;
} scanner_t;

typedef struct {
    int kind;
    const void *ptr;
    size_t size;
    json_int_t integer;
    double real;
} jsonp_pack_arg;

typedef struct {
    int kind;
    void *ptr;
} jsonp_unpack_arg;

enum {
    JSONP_PACK_ARG_CSTR = 1,
    JSONP_PACK_ARG_INT = 2,
    JSONP_PACK_ARG_SIZE = 3,
    JSONP_PACK_ARG_DOUBLE = 4,
    JSONP_PACK_ARG_JSON = 5,
};

enum {
    JSONP_UNPACK_ARG_KEY = 1,
    JSONP_UNPACK_ARG_STRING = 2,
    JSONP_UNPACK_ARG_SIZE = 3,
    JSONP_UNPACK_ARG_INT = 4,
    JSONP_UNPACK_ARG_JSON_INT = 5,
    JSONP_UNPACK_ARG_DOUBLE = 6,
    JSONP_UNPACK_ARG_JSON = 7,
};

void *jsonp_malloc(size_t size);
void jsonp_free(void *ptr);
void jsonp_error_init(json_error_t *error, const char *source);
void jsonp_error_set_source(json_error_t *error, const char *source);
void jsonp_error_set(json_error_t *error, int line, int column, size_t position,
                     enum json_error_code code, const char *msg, ...);

json_t *jsonp_pack_marshaled(json_error_t *error, size_t flags, const char *fmt,
                             const jsonp_pack_arg *args, size_t args_len);
int jsonp_unpack_marshaled(json_t *root, json_error_t *error, size_t flags,
                           const char *fmt, const jsonp_unpack_arg *args,
                           size_t args_len);

#define token(scanner) ((scanner)->token.token)

typedef struct {
    jsonp_pack_arg *args;
    size_t index;
    va_list *ap;
} pack_writer_t;

typedef struct {
    jsonp_unpack_arg *args;
    size_t index;
    va_list *ap;
} unpack_writer_t;

static void scanner_init(scanner_t *scanner, const char *fmt) {
    scanner->fmt = fmt;
    memset(&scanner->prev_token, 0, sizeof(token_t));
    memset(&scanner->token, 0, sizeof(token_t));
    memset(&scanner->next_token, 0, sizeof(token_t));
    scanner->line = 1;
    scanner->column = 0;
    scanner->pos = 0;
}

static void next_token(scanner_t *scanner) {
    const char *t;

    scanner->prev_token = scanner->token;
    if (scanner->next_token.line) {
        scanner->token = scanner->next_token;
        scanner->next_token.line = 0;
        return;
    }

    if (!token(scanner) && !*scanner->fmt)
        return;

    t = scanner->fmt;
    scanner->column++;
    scanner->pos++;

    while (*t == ' ' || *t == '\t' || *t == '\n' || *t == ',' || *t == ':') {
        if (*t == '\n') {
            scanner->line++;
            scanner->column = 1;
        } else {
            scanner->column++;
        }

        scanner->pos++;
        t++;
    }

    scanner->token.token = *t;
    scanner->token.line = scanner->line;
    scanner->token.column = scanner->column;
    scanner->token.pos = scanner->pos;

    if (*t)
        t++;
    scanner->fmt = t;
}

static void prev_token(scanner_t *scanner) {
    scanner->next_token = scanner->token;
    scanner->token = scanner->prev_token;
}

static void set_oom_error(json_error_t *error) {
    jsonp_error_init(error, "<internal>");
    jsonp_error_set(error, -1, -1, 0, json_error_out_of_memory, "Out of memory");
    jsonp_error_set_source(error, "<internal>");
}

static void *alloc_marshaled(size_t count, size_t item_size) {
    size_t size;

    if (count == 0)
        return NULL;

    if (count > SIZE_MAX / item_size)
        return NULL;

    size = count * item_size;
    return jsonp_malloc(size);
}

static void pack_emit_cstr(pack_writer_t *writer, const char *value) {
    if (writer->args) {
        writer->args[writer->index].kind = JSONP_PACK_ARG_CSTR;
        writer->args[writer->index].ptr = value;
        writer->args[writer->index].size = 0;
        writer->args[writer->index].integer = 0;
        writer->args[writer->index].real = 0.0;
    }
    writer->index++;
}

static void pack_emit_int(pack_writer_t *writer, int value) {
    if (writer->args) {
        writer->args[writer->index].kind = JSONP_PACK_ARG_INT;
        writer->args[writer->index].ptr = NULL;
        writer->args[writer->index].size = 0;
        writer->args[writer->index].integer = (json_int_t)value;
        writer->args[writer->index].real = 0.0;
    }
    writer->index++;
}

static void pack_emit_json_int(pack_writer_t *writer, json_int_t value) {
    if (writer->args) {
        writer->args[writer->index].kind = JSONP_PACK_ARG_INT;
        writer->args[writer->index].ptr = NULL;
        writer->args[writer->index].size = 0;
        writer->args[writer->index].integer = value;
        writer->args[writer->index].real = 0.0;
    }
    writer->index++;
}

static void pack_emit_size(pack_writer_t *writer, size_t value) {
    if (writer->args) {
        writer->args[writer->index].kind = JSONP_PACK_ARG_SIZE;
        writer->args[writer->index].ptr = NULL;
        writer->args[writer->index].size = value;
        writer->args[writer->index].integer = 0;
        writer->args[writer->index].real = 0.0;
    }
    writer->index++;
}

static void pack_emit_double(pack_writer_t *writer, double value) {
    if (writer->args) {
        writer->args[writer->index].kind = JSONP_PACK_ARG_DOUBLE;
        writer->args[writer->index].ptr = NULL;
        writer->args[writer->index].size = 0;
        writer->args[writer->index].integer = 0;
        writer->args[writer->index].real = value;
    }
    writer->index++;
}

static void pack_emit_json(pack_writer_t *writer, json_t *value) {
    if (writer->args) {
        writer->args[writer->index].kind = JSONP_PACK_ARG_JSON;
        writer->args[writer->index].ptr = value;
        writer->args[writer->index].size = 0;
        writer->args[writer->index].integer = 0;
        writer->args[writer->index].real = 0.0;
    }
    writer->index++;
}

static void unpack_emit(unpack_writer_t *writer, int kind, void *value) {
    if (writer->args) {
        writer->args[writer->index].kind = kind;
        writer->args[writer->index].ptr = value;
    }
    writer->index++;
}

static int marshal_pack_value(scanner_t *scanner, pack_writer_t *writer);
static int marshal_unpack_value(scanner_t *scanner, unpack_writer_t *writer, size_t flags);

static int pack_read_string(scanner_t *scanner, pack_writer_t *writer, int optional) {
    char modifier;

    next_token(scanner);
    modifier = token(scanner);
    prev_token(scanner);

    if (modifier != '#' && modifier != '%' && modifier != '+') {
        if (writer->ap) {
            pack_emit_cstr(writer, va_arg(*writer->ap, const char *));
        } else {
            pack_emit_cstr(writer, NULL);
        }
        return 0;
    }

    if (optional)
        return -1;

    while (1) {
        if (writer->ap) {
            pack_emit_cstr(writer, va_arg(*writer->ap, const char *));
        } else {
            pack_emit_cstr(writer, NULL);
        }

        next_token(scanner);
        if (token(scanner) == '#') {
            if (writer->ap)
                pack_emit_int(writer, va_arg(*writer->ap, int));
            else
                pack_emit_int(writer, 0);
        } else if (token(scanner) == '%') {
            if (writer->ap)
                pack_emit_size(writer, va_arg(*writer->ap, size_t));
            else
                pack_emit_size(writer, 0);
        } else {
            prev_token(scanner);
        }

        next_token(scanner);
        if (token(scanner) != '+') {
            prev_token(scanner);
            break;
        }
    }

    return 0;
}

static int marshal_pack_object(scanner_t *scanner, pack_writer_t *writer) {
    next_token(scanner);

    while (token(scanner) != '}') {
        if (!token(scanner) || token(scanner) != 's')
            return -1;

        if (pack_read_string(scanner, writer, 0) != 0)
            return -1;

        next_token(scanner);
        if (marshal_pack_value(scanner, writer) != 0)
            return -1;
        next_token(scanner);
    }

    return 0;
}

static int marshal_pack_array(scanner_t *scanner, pack_writer_t *writer) {
    next_token(scanner);

    while (token(scanner) != ']') {
        if (!token(scanner))
            return -1;

        if (marshal_pack_value(scanner, writer) != 0)
            return -1;
        next_token(scanner);
    }

    return 0;
}

static int marshal_pack_object_inter(scanner_t *scanner, pack_writer_t *writer) {
    next_token(scanner);
    if (token(scanner) != '?' && token(scanner) != '*')
        prev_token(scanner);

    if (writer->ap) {
        pack_emit_json(writer, va_arg(*writer->ap, json_t *));
    } else {
        pack_emit_json(writer, NULL);
    }

    return 0;
}

static int marshal_pack_value(scanner_t *scanner, pack_writer_t *writer) {
    switch (token(scanner)) {
        case '{':
            return marshal_pack_object(scanner, writer);

        case '[':
            return marshal_pack_array(scanner, writer);

        case 's':
            next_token(scanner);
            if (token(scanner) != '?' && token(scanner) != '*')
                prev_token(scanner);
            return pack_read_string(scanner, writer, token(scanner) == '?' || token(scanner) == '*');

        case 'n':
            return 0;

        case 'b':
        case 'i':
            if (writer->ap)
                pack_emit_int(writer, va_arg(*writer->ap, int));
            else
                pack_emit_int(writer, 0);
            return 0;

        case 'I':
            if (writer->ap)
                pack_emit_json_int(writer, va_arg(*writer->ap, json_int_t));
            else
                pack_emit_json_int(writer, 0);
            return 0;

        case 'f':
            if (writer->ap)
                pack_emit_double(writer, va_arg(*writer->ap, double));
            else
                pack_emit_double(writer, 0.0);
            return 0;

        case 'O':
        case 'o':
            return marshal_pack_object_inter(scanner, writer);

        default:
            return -1;
    }
}

static int marshal_pack_args(const char *fmt, va_list *ap, jsonp_pack_arg *args,
                             size_t *args_len) {
    scanner_t scanner;
    pack_writer_t writer;

    writer.args = args;
    writer.index = 0;
    writer.ap = ap;

    scanner_init(&scanner, fmt);
    next_token(&scanner);
    if (marshal_pack_value(&scanner, &writer) != 0) {
        *args_len = writer.index;
        return -1;
    }

    *args_len = writer.index;
    return 0;
}

static int marshal_unpack_object(scanner_t *scanner, unpack_writer_t *writer, size_t flags) {
    int strict = 0;

    next_token(scanner);
    while (token(scanner) != '}') {
        if (strict != 0 || !token(scanner))
            return -1;

        if (token(scanner) == '!' || token(scanner) == '*') {
            strict = (token(scanner) == '!') ? 1 : -1;
            next_token(scanner);
            continue;
        }

        if (token(scanner) != 's')
            return -1;

        if (writer->ap)
            unpack_emit(writer, JSONP_UNPACK_ARG_KEY, va_arg(*writer->ap, void *));
        else
            unpack_emit(writer, JSONP_UNPACK_ARG_KEY, NULL);

        next_token(scanner);
        if (token(scanner) == '?')
            next_token(scanner);

        if (marshal_unpack_value(scanner, writer, flags) != 0)
            return -1;
        next_token(scanner);
    }

    return 0;
}

static int marshal_unpack_array(scanner_t *scanner, unpack_writer_t *writer, size_t flags) {
    int strict = 0;

    next_token(scanner);
    while (token(scanner) != ']') {
        if (strict != 0 || !token(scanner))
            return -1;

        if (token(scanner) == '!' || token(scanner) == '*') {
            strict = (token(scanner) == '!') ? 1 : -1;
            next_token(scanner);
            continue;
        }

        if (!strchr("{[siIbfFOon", token(scanner)))
            return -1;

        if (marshal_unpack_value(scanner, writer, flags) != 0)
            return -1;
        next_token(scanner);
    }

    return 0;
}

static int marshal_unpack_value(scanner_t *scanner, unpack_writer_t *writer, size_t flags) {
    switch (token(scanner)) {
        case '{':
            return marshal_unpack_object(scanner, writer, flags);

        case '[':
            return marshal_unpack_array(scanner, writer, flags);

        case 's':
            if (!(flags & JSON_VALIDATE_ONLY)) {
                if (writer->ap)
                    unpack_emit(writer, JSONP_UNPACK_ARG_STRING, va_arg(*writer->ap, void *));
                else
                    unpack_emit(writer, JSONP_UNPACK_ARG_STRING, NULL);

                next_token(scanner);
                if (token(scanner) == '%') {
                    if (writer->ap)
                        unpack_emit(writer, JSONP_UNPACK_ARG_SIZE,
                                    va_arg(*writer->ap, void *));
                    else
                        unpack_emit(writer, JSONP_UNPACK_ARG_SIZE, NULL);
                } else {
                    prev_token(scanner);
                }
            }
            return 0;

        case 'i':
            if (!(flags & JSON_VALIDATE_ONLY)) {
                if (writer->ap)
                    unpack_emit(writer, JSONP_UNPACK_ARG_INT, va_arg(*writer->ap, void *));
                else
                    unpack_emit(writer, JSONP_UNPACK_ARG_INT, NULL);
            }
            return 0;

        case 'I':
            if (!(flags & JSON_VALIDATE_ONLY)) {
                if (writer->ap)
                    unpack_emit(writer, JSONP_UNPACK_ARG_JSON_INT,
                                va_arg(*writer->ap, void *));
                else
                    unpack_emit(writer, JSONP_UNPACK_ARG_JSON_INT, NULL);
            }
            return 0;

        case 'b':
            if (!(flags & JSON_VALIDATE_ONLY)) {
                if (writer->ap)
                    unpack_emit(writer, JSONP_UNPACK_ARG_INT, va_arg(*writer->ap, void *));
                else
                    unpack_emit(writer, JSONP_UNPACK_ARG_INT, NULL);
            }
            return 0;

        case 'f':
        case 'F':
            if (!(flags & JSON_VALIDATE_ONLY)) {
                if (writer->ap)
                    unpack_emit(writer, JSONP_UNPACK_ARG_DOUBLE, va_arg(*writer->ap, void *));
                else
                    unpack_emit(writer, JSONP_UNPACK_ARG_DOUBLE, NULL);
            }
            return 0;

        case 'O':
        case 'o':
            if (!(flags & JSON_VALIDATE_ONLY)) {
                if (writer->ap)
                    unpack_emit(writer, JSONP_UNPACK_ARG_JSON, va_arg(*writer->ap, void *));
                else
                    unpack_emit(writer, JSONP_UNPACK_ARG_JSON, NULL);
            }
            return 0;

        case 'n':
            return 0;

        default:
            return -1;
    }
}

static int marshal_unpack_args(const char *fmt, size_t flags, va_list *ap,
                               jsonp_unpack_arg *args, size_t *args_len) {
    scanner_t scanner;
    unpack_writer_t writer;

    writer.args = args;
    writer.index = 0;
    writer.ap = ap;

    scanner_init(&scanner, fmt);
    next_token(&scanner);
    if (marshal_unpack_value(&scanner, &writer, flags) != 0) {
        *args_len = writer.index;
        return -1;
    }

    *args_len = writer.index;
    return 0;
}

json_t *json_pack(const char *fmt, ...) {
    json_t *value;
    va_list ap;

    va_start(ap, fmt);
    value = json_vpack_ex(NULL, 0, fmt, ap);
    va_end(ap);

    return value;
}

json_t *json_pack_ex(json_error_t *error, size_t flags, const char *fmt, ...) {
    json_t *value;
    va_list ap;

    va_start(ap, fmt);
    value = json_vpack_ex(error, flags, fmt, ap);
    va_end(ap);

    return value;
}

json_t *json_vpack_ex(json_error_t *error, size_t flags, const char *fmt, va_list ap) {
    jsonp_pack_arg *args = NULL;
    size_t args_len = 0;
    json_t *value;
    va_list aq;

    if (!fmt || !*fmt)
        return jsonp_pack_marshaled(error, flags, fmt, NULL, 0);

    if (marshal_pack_args(fmt, NULL, NULL, &args_len) != 0 && args_len == 0)
        return jsonp_pack_marshaled(error, flags, fmt, NULL, 0);

    args = alloc_marshaled(args_len, sizeof(*args));
    if (args_len && !args) {
        set_oom_error(error);
        return NULL;
    }

    va_copy(aq, ap);
    marshal_pack_args(fmt, &aq, args, &args_len);
    va_end(aq);

    value = jsonp_pack_marshaled(error, flags, fmt, args, args_len);
    jsonp_free(args);
    return value;
}

int json_unpack(json_t *root, const char *fmt, ...) {
    int result;
    va_list ap;

    va_start(ap, fmt);
    result = json_vunpack_ex(root, NULL, 0, fmt, ap);
    va_end(ap);

    return result;
}

int json_unpack_ex(json_t *root, json_error_t *error, size_t flags, const char *fmt, ...) {
    int result;
    va_list ap;

    va_start(ap, fmt);
    result = json_vunpack_ex(root, error, flags, fmt, ap);
    va_end(ap);

    return result;
}

int json_vunpack_ex(json_t *root, json_error_t *error, size_t flags, const char *fmt,
                    va_list ap) {
    jsonp_unpack_arg *args = NULL;
    size_t args_len = 0;
    int result;
    va_list aq;

    if (!root || !fmt || !*fmt)
        return jsonp_unpack_marshaled(root, error, flags, fmt, NULL, 0);

    if (marshal_unpack_args(fmt, flags, NULL, NULL, &args_len) != 0 && args_len == 0)
        return jsonp_unpack_marshaled(root, error, flags, fmt, NULL, 0);

    args = alloc_marshaled(args_len, sizeof(*args));
    if (args_len && !args) {
        set_oom_error(error);
        return -1;
    }

    va_copy(aq, ap);
    marshal_unpack_args(fmt, flags, &aq, args, &args_len);
    va_end(aq);

    result = jsonp_unpack_marshaled(root, error, flags, fmt, args, args_len);
    jsonp_free(args);
    return result;
}
