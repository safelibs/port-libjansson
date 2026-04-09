#define _GNU_SOURCE

#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#ifndef LIBJANSSON_SAFE_MULTIARCH
#define LIBJANSSON_SAFE_MULTIARCH "x86_64-linux-gnu"
#endif

static int (*real_access_fn)(const char *, int);
static int (*real_faccessat_fn)(int, const char *, int, int);
static FILE *(*real_fopen_fn)(const char *, const char *);
static FILE *(*real_fopen64_fn)(const char *, const char *);
static int (*real_lstat_fn)(const char *, struct stat *);
static int (*real_open_fn)(const char *, int, ...);
static int (*real_open64_fn)(const char *, int, ...);
static int (*real_openat_fn)(int, const char *, int, ...);
static int (*real_openat64_fn)(int, const char *, int, ...);
static int (*real_stat_fn)(const char *, struct stat *);
static int (*real_statx_fn)(int, const char *, int, unsigned int, struct statx *);
static int (*real_xstat_fn)(int, const char *, struct stat *);
static int (*real_xstat64_fn)(int, const char *, struct stat64 *);
static int (*real_lxstat_fn)(int, const char *, struct stat *);
static int (*real_lxstat64_fn)(int, const char *, struct stat64 *);

static const char *interesting_prefixes[] = {
    "/usr/include/jansson.h",
    "/usr/include/jansson_config.h",
    "/usr/lib/" LIBJANSSON_SAFE_MULTIARCH "/libjansson",
};

static void init_real_functions(void) {
    if (!real_access_fn)
        real_access_fn = dlsym(RTLD_NEXT, "access");
    if (!real_faccessat_fn)
        real_faccessat_fn = dlsym(RTLD_NEXT, "faccessat");
    if (!real_fopen_fn)
        real_fopen_fn = dlsym(RTLD_NEXT, "fopen");
    if (!real_fopen64_fn)
        real_fopen64_fn = dlsym(RTLD_NEXT, "fopen64");
    if (!real_lstat_fn)
        real_lstat_fn = dlsym(RTLD_NEXT, "lstat");
    if (!real_open_fn)
        real_open_fn = dlsym(RTLD_NEXT, "open");
    if (!real_open64_fn)
        real_open64_fn = dlsym(RTLD_NEXT, "open64");
    if (!real_openat_fn)
        real_openat_fn = dlsym(RTLD_NEXT, "openat");
    if (!real_openat64_fn)
        real_openat64_fn = dlsym(RTLD_NEXT, "openat64");
    if (!real_stat_fn)
        real_stat_fn = dlsym(RTLD_NEXT, "stat");
    if (!real_statx_fn)
        real_statx_fn = dlsym(RTLD_NEXT, "statx");
    if (!real_xstat_fn)
        real_xstat_fn = dlsym(RTLD_NEXT, "__xstat");
    if (!real_xstat64_fn)
        real_xstat64_fn = dlsym(RTLD_NEXT, "__xstat64");
    if (!real_lxstat_fn)
        real_lxstat_fn = dlsym(RTLD_NEXT, "__lxstat");
    if (!real_lxstat64_fn)
        real_lxstat64_fn = dlsym(RTLD_NEXT, "__lxstat64");
}

static bool starts_with(const char *value, const char *prefix) {
    size_t prefix_len;

    if (!value || !prefix)
        return false;

    prefix_len = strlen(prefix);
    return strncmp(value, prefix, prefix_len) == 0;
}

static bool is_interesting_path(const char *path) {
    size_t i;

    if (!path || path[0] != '/')
        return false;

    for (i = 0; i < sizeof(interesting_prefixes) / sizeof(interesting_prefixes[0]); i++) {
        if (starts_with(path, interesting_prefixes[i]))
            return true;
    }

    return false;
}

static const char *stage_root(void) {
    static int initialized = 0;
    static char root_buf[PATH_MAX];

    if (!initialized) {
        const char *env_root = getenv("LIBJANSSON_SAFE_STAGE_ROOT");
        init_real_functions();
        if (env_root && env_root[0]) {
            snprintf(root_buf, sizeof(root_buf), "%s", env_root);
        } else {
            const char *home = getenv("HOME");
            char cfg_path[PATH_MAX];
            FILE *cfg;
            char line[PATH_MAX];

            if (home && home[0]) {
                snprintf(cfg_path, sizeof(cfg_path), "%s/.dpkg.cfg", home);
                cfg = real_fopen_fn ? real_fopen_fn(cfg_path, "r") : NULL;
                if (cfg) {
                    while (fgets(line, sizeof(line), cfg)) {
                        if (starts_with(line, "root=")) {
                            size_t len;
                            snprintf(root_buf, sizeof(root_buf), "%s", line + 5);
                            len = strlen(root_buf);
                            while (len > 0 &&
                                   (root_buf[len - 1] == '\n' || root_buf[len - 1] == '\r')) {
                                root_buf[--len] = '\0';
                            }
                        }
                    }
                    fclose(cfg);
                }
            }
        }
        initialized = 1;
    }

    return root_buf[0] ? root_buf : NULL;
}

static const char *redirect_path(const char *path, char *buffer, size_t buffer_len) {
    const char *root;

    init_real_functions();

    if (!is_interesting_path(path))
        return path;

    if (real_access_fn && real_access_fn(path, F_OK) == 0)
        return path;

    root = stage_root();
    if (!root)
        return path;

    if (snprintf(buffer, buffer_len, "%s%s", root, path) >= (int)buffer_len)
        return path;

    if (real_access_fn && real_access_fn(buffer, F_OK) == 0)
        return buffer;

    return path;
}

static const char *normalize_relative_path(int dirfd, const char *path, char *buffer,
                                           size_t buffer_len) {
    char dirfd_path[PATH_MAX];
    ssize_t len;

    if (!path || path[0] == '/' || dirfd == AT_FDCWD)
        return path;

    snprintf(dirfd_path, sizeof(dirfd_path), "/proc/self/fd/%d", dirfd);
    len = readlink(dirfd_path, buffer, buffer_len - 1);
    if (len < 0)
        return path;

    buffer[len] = '\0';
    if (snprintf(buffer + len, buffer_len - (size_t)len, "/%s", path) >=
        (int)(buffer_len - (size_t)len))
        return path;

    return buffer;
}

static mode_t read_mode_arg(int flags, va_list args) {
    if (flags & O_CREAT)
        return (mode_t)va_arg(args, int);
    return 0;
}

int access(const char *path, int mode) {
    char redirected[PATH_MAX];
    init_real_functions();
    return real_access_fn(redirect_path(path, redirected, sizeof(redirected)), mode);
}

int faccessat(int dirfd, const char *path, int mode, int flags) {
    char absolute[PATH_MAX];
    char redirected[PATH_MAX];
    const char *candidate = normalize_relative_path(dirfd, path, absolute, sizeof(absolute));

    init_real_functions();
    if (candidate != path)
        return real_access_fn(redirect_path(candidate, redirected, sizeof(redirected)), mode);

    return real_faccessat_fn(dirfd, redirect_path(path, redirected, sizeof(redirected)), mode,
                             flags);
}

FILE *fopen(const char *path, const char *mode) {
    char redirected[PATH_MAX];
    init_real_functions();
    return real_fopen_fn(redirect_path(path, redirected, sizeof(redirected)), mode);
}

FILE *fopen64(const char *path, const char *mode) {
    char redirected[PATH_MAX];
    init_real_functions();
    return real_fopen64_fn(redirect_path(path, redirected, sizeof(redirected)), mode);
}

int lstat(const char *path, struct stat *st) {
    char redirected[PATH_MAX];
    init_real_functions();
    return real_lstat_fn(redirect_path(path, redirected, sizeof(redirected)), st);
}

int open(const char *path, int flags, ...) {
    char redirected[PATH_MAX];
    va_list args;
    mode_t mode;

    init_real_functions();
    va_start(args, flags);
    mode = read_mode_arg(flags, args);
    va_end(args);

    return real_open_fn(redirect_path(path, redirected, sizeof(redirected)), flags, mode);
}

int open64(const char *path, int flags, ...) {
    char redirected[PATH_MAX];
    va_list args;
    mode_t mode;

    init_real_functions();
    va_start(args, flags);
    mode = read_mode_arg(flags, args);
    va_end(args);

    return real_open64_fn(redirect_path(path, redirected, sizeof(redirected)), flags, mode);
}

int openat(int dirfd, const char *path, int flags, ...) {
    char absolute[PATH_MAX];
    char redirected[PATH_MAX];
    const char *candidate = normalize_relative_path(dirfd, path, absolute, sizeof(absolute));
    va_list args;
    mode_t mode;

    init_real_functions();
    va_start(args, flags);
    mode = read_mode_arg(flags, args);
    va_end(args);

    if (candidate != path)
        return real_open_fn(redirect_path(candidate, redirected, sizeof(redirected)), flags, mode);

    return real_openat_fn(dirfd, redirect_path(path, redirected, sizeof(redirected)), flags, mode);
}

int openat64(int dirfd, const char *path, int flags, ...) {
    char absolute[PATH_MAX];
    char redirected[PATH_MAX];
    const char *candidate = normalize_relative_path(dirfd, path, absolute, sizeof(absolute));
    va_list args;
    mode_t mode;

    init_real_functions();
    va_start(args, flags);
    mode = read_mode_arg(flags, args);
    va_end(args);

    if (candidate != path)
        return real_open64_fn(redirect_path(candidate, redirected, sizeof(redirected)), flags,
                              mode);

    return real_openat64_fn(dirfd, redirect_path(path, redirected, sizeof(redirected)), flags,
                            mode);
}

int stat(const char *path, struct stat *st) {
    char redirected[PATH_MAX];
    init_real_functions();
    return real_stat_fn(redirect_path(path, redirected, sizeof(redirected)), st);
}

int statx(int dirfd, const char *path, int flags, unsigned int mask, struct statx *buf) {
    char absolute[PATH_MAX];
    char redirected[PATH_MAX];
    const char *candidate = normalize_relative_path(dirfd, path, absolute, sizeof(absolute));

    init_real_functions();
    if (candidate != path)
        return real_statx_fn(AT_FDCWD, redirect_path(candidate, redirected, sizeof(redirected)),
                             flags, mask, buf);

    return real_statx_fn(dirfd, redirect_path(path, redirected, sizeof(redirected)), flags, mask,
                         buf);
}

int __xstat(int ver, const char *path, struct stat *st) {
    char redirected[PATH_MAX];
    init_real_functions();
    return real_xstat_fn(ver, redirect_path(path, redirected, sizeof(redirected)), st);
}

int __xstat64(int ver, const char *path, struct stat64 *st) {
    char redirected[PATH_MAX];
    init_real_functions();
    return real_xstat64_fn(ver, redirect_path(path, redirected, sizeof(redirected)), st);
}

int __lxstat(int ver, const char *path, struct stat *st) {
    char redirected[PATH_MAX];
    init_real_functions();
    return real_lxstat_fn(ver, redirect_path(path, redirected, sizeof(redirected)), st);
}

int __lxstat64(int ver, const char *path, struct stat64 *st) {
    char redirected[PATH_MAX];
    init_real_functions();
    return real_lxstat64_fn(ver, redirect_path(path, redirected, sizeof(redirected)), st);
}
