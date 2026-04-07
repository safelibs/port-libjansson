/*
 * Copyright (c) 2009-2016 Petri Lehtinen <petri@digip.org>
 *
 * Jansson is free software; you can redistribute it and/or modify
 * it under the terms of the MIT license. See LICENSE for details.
 */

#ifndef UTIL_H
#define UTIL_H

#include <locale.h>
#include <stdio.h>
#include <stdlib.h>

#include <jansson.h>

#define failhdr fprintf(stderr, "%s:%d: ", __FILE__, __LINE__)

#define fail(msg)                                                                        \
    do {                                                                                 \
        failhdr;                                                                         \
        fprintf(stderr, "%s\n", msg);                                                    \
        exit(1);                                                                         \
    } while (0)

typedef struct alloc_tracker_t {
    json_malloc_t malloc_fn;
    json_free_t free_fn;
    size_t allocations;
    size_t frees;
} alloc_tracker_t;

static alloc_tracker_t *active_alloc_tracker = NULL;

static JANSSON_ATTRS((unused)) void *tracking_malloc(size_t size) {
    alloc_tracker_t *tracker = active_alloc_tracker;
    void *ptr;

    if (!tracker)
        fail("tracking_malloc called without an active tracker");

    ptr = tracker->malloc_fn(size);
    if (ptr)
        tracker->allocations++;

    return ptr;
}

static JANSSON_ATTRS((unused)) void tracking_free(void *ptr) {
    alloc_tracker_t *tracker = active_alloc_tracker;

    if (!tracker)
        fail("tracking_free called without an active tracker");

    if (ptr)
        tracker->frees++;

    tracker->free_fn(ptr);
}

static JANSSON_ATTRS((unused)) void alloc_tracker_begin(alloc_tracker_t *tracker) {
    if (!tracker)
        fail("alloc_tracker_begin called with NULL tracker");
    if (active_alloc_tracker)
        fail("allocation tracking is already active");

    tracker->allocations = 0;
    tracker->frees = 0;
    json_get_alloc_funcs(&tracker->malloc_fn, &tracker->free_fn);
    active_alloc_tracker = tracker;
    json_set_alloc_funcs(tracking_malloc, tracking_free);
}

static JANSSON_ATTRS((unused)) void alloc_tracker_check(alloc_tracker_t *tracker) {
    if (tracker->allocations != tracker->frees) {
        failhdr;
        fprintf(stderr, "leaked Jansson allocations: %zu allocation(s), %zu free(s)\n",
                tracker->allocations, tracker->frees);
        exit(1);
    }
}

static JANSSON_ATTRS((unused)) void alloc_tracker_end(alloc_tracker_t *tracker) {
    if (!tracker)
        fail("alloc_tracker_end called with NULL tracker");
    if (active_alloc_tracker != tracker)
        fail("allocation tracking state mismatch");

    json_set_alloc_funcs(tracker->malloc_fn, tracker->free_fn);
    active_alloc_tracker = NULL;
}

#define assert_no_alloc_leaks(code)                                                      \
    do {                                                                                 \
        alloc_tracker_t tracker_;                                                        \
        alloc_tracker_begin(&tracker_);                                                  \
        do {                                                                             \
            code                                                                         \
        } while (0);                                                                     \
        alloc_tracker_check(&tracker_);                                                  \
        alloc_tracker_end(&tracker_);                                                    \
    } while (0)

/* Assumes json_error_t error */
#define check_errors(code_, texts_, num_, source_, line_, column_, position_)            \
    do {                                                                                 \
        int i_, found_ = 0;                                                              \
        if (json_error_code(&error) != code_) {                                          \
            failhdr;                                                                     \
            fprintf(stderr, "code: %d != %d\n", json_error_code(&error), code_);         \
            exit(1);                                                                     \
        }                                                                                \
        for (i_ = 0; i_ < num_; i_++) {                                                  \
            if (strcmp(error.text, texts_[i_]) == 0) {                                   \
                found_ = 1;                                                              \
                break;                                                                   \
            }                                                                            \
        }                                                                                \
        if (!found_) {                                                                   \
            failhdr;                                                                     \
            if (num_ == 1) {                                                             \
                fprintf(stderr, "text: \"%s\" != \"%s\"\n", error.text, texts_[0]);      \
            } else {                                                                     \
                fprintf(stderr, "text: \"%s\" does not match\n", error.text);            \
            }                                                                            \
            exit(1);                                                                     \
        }                                                                                \
        if (strcmp(error.source, source_) != 0) {                                        \
            failhdr;                                                                     \
                                                                                         \
            fprintf(stderr, "source: \"%s\" != \"%s\"\n", error.source, source_);        \
            exit(1);                                                                     \
        }                                                                                \
        if (error.line != line_) {                                                       \
            failhdr;                                                                     \
            fprintf(stderr, "line: %d != %d\n", error.line, line_);                      \
            exit(1);                                                                     \
        }                                                                                \
        if (error.column != column_) {                                                   \
            failhdr;                                                                     \
            fprintf(stderr, "column: %d != %d\n", error.column, column_);                \
            exit(1);                                                                     \
        }                                                                                \
        if (error.position != position_) {                                               \
            failhdr;                                                                     \
            fprintf(stderr, "position: %d != %d\n", error.position, position_);          \
            exit(1);                                                                     \
        }                                                                                \
    } while (0)

/* Assumes json_error_t error */
#define check_error(code_, text_, source_, line_, column_, position_)                    \
    check_errors(code_, &text_, 1, source_, line_, column_, position_)

static void run_tests();

int main() {
    setlocale(LC_ALL, "");
    run_tests();
    return 0;
}

#endif
