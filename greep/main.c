//
//  main.c
//  greep
//
//  Created by Peter Richardson on 12/3/21.
//

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <pthread.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include <time.h>

#include "search_algorithms/search_algorithms.h"
#include "options.h"

typedef struct threadargs_t {
    const char *search_word;
    const char *filename;
    search_alg_t search_alg;
    callback_t found_callback;
    int timing;
    long *elapsed_usec_out;
    unsigned long *bytes_out;
    int *ok_out;
} threadargs_t;

// Read all of fd into a growable heap buffer. Used for pipes/ttys, which
// can't be mmap'd and may block waiting for data (e.g. interactive stdin).
static void *slurp_fd(int fd, size_t *out_len) {
    size_t cap = 1 << 16;
    size_t len = 0;
    char *buf = malloc(cap);
    if (!buf) return NULL;

    ssize_t n;
    while ((n = read(fd, buf + len, cap - len)) > 0) {
        len += (size_t) n;
        if (len == cap) {
            cap *= 2;
            char *grown = realloc(buf, cap);
            if (!grown) {
                free(buf);
                return NULL;
            }
            buf = grown;
        }
    }
    *out_len = len;
    return buf;
}

void *threaded_find(void *arg) {
    threadargs_t thread_args = *(threadargs_t *) arg;

    int fdin;
    struct stat sbuf;
    void *src;
    int is_mmapped = 0;
    unsigned long src_len;

    if ((fdin = open(thread_args.filename, O_RDONLY)) < 0) {
        fprintf(stderr, "# ERROR: Unable to open file '%s' for reading\n", thread_args.filename);
        return (void*) 0;
    }
    if (fstat(fdin, &sbuf) < 0) {
        fprintf(stderr, "# ERROR: fstat error reading '%s'\n", thread_args.filename);
        close(fdin);
        return (void*) 0;
    }

    if (S_ISREG(sbuf.st_mode)) {
        // Regular file: mmap it.
        if ((src = mmap(0, sbuf.st_size, PROT_READ, MAP_SHARED, fdin, 0)) == MAP_FAILED) {
            fprintf(stderr, "# ERROR: mmap error for '%s'\n", thread_args.filename);
            close(fdin);
            return (void*) 0;
        }
        is_mmapped = 1;
        src_len = sbuf.st_size;
    } else {
        // Pipe, FIFO, or TTY (e.g. /dev/stdin with no redirection): can't be
        // mmap'd, so read it instead. This blocks until EOF, matching the
        // behavior of standard CLI tools when waiting on interactive stdin.
        size_t len = 0;
        src = slurp_fd(fdin, &len);
        if (!src) {
            fprintf(stderr, "# ERROR: out of memory reading '%s'\n", thread_args.filename);
            close(fdin);
            return (void*) 0;
        }
        src_len = (unsigned long) len;
    }

    *thread_args.bytes_out = src_len;

    // ACTUALLY DO THE WORK.   Call the specified search algorithm.
    if (thread_args.timing) {
        struct timespec t0, t1;
        clock_gettime(CLOCK_MONOTONIC, &t0);
        (thread_args.search_alg)(thread_args.search_word, thread_args.filename, src, src_len, &thread_args.found_callback);
        clock_gettime(CLOCK_MONOTONIC, &t1);

        long elapsed_usec = (t1.tv_sec - t0.tv_sec) * 1000000L + (t1.tv_nsec - t0.tv_nsec) / 1000L;
        *thread_args.elapsed_usec_out = elapsed_usec;
        fprintf(stderr, "#TIMING %8ld %s\n", elapsed_usec, thread_args.filename);
    } else {
        (thread_args.search_alg)(thread_args.search_word, thread_args.filename, src, src_len, &thread_args.found_callback);
    }

    *thread_args.ok_out = 1;

    if (is_mmapped) {
        munmap(src, src_len);
    } else {
        free(src);
    }
    close(fdin);
    return 0;
}

// Run this code whenever a match is found
void found_callback(const char *filename, unsigned long line_num, const char *line_start, const char *line_end) {
    printf("%s:%lu ", filename, line_num);
    for ( char *p = (char*) line_start; p < line_end; p++) {
        putchar(*p);
    }
    putchar('\n');
}

int main(int argc, char *argv[]) {
    arguments_t args = ARGUMENT_DEFAULT_VALUES;
    parse_args(argc, argv, &args);

    search_alg_t chosen_alg = find_algorithm(args.algorithm_code);

    if (args.verbose) {
        fprintf(stderr, "# Searching for '%s'\n", args.search_word);
    }

    pthread_t threads[args.filecount];
    threadargs_t payloads[args.filecount];
    long elapsed_usec[args.filecount];
    unsigned long bytes_searched[args.filecount];
    int file_ok[args.filecount];

    for (int i = 0; i < args.filecount; i++ ) {
        elapsed_usec[i] = 0;
        bytes_searched[i] = 0;
        file_ok[i] = 0;
        threadargs_t thread_args = {
            .search_word = args.search_word,
            .filename = args.filenames[i],
            .search_alg = chosen_alg,
            .found_callback = found_callback,
            .timing = args.timing,
            .elapsed_usec_out = &elapsed_usec[i],
            .bytes_out = &bytes_searched[i],
            .ok_out = &file_ok[i]
        };
        payloads[i] = thread_args;
    }

    for (int i = 0; i < args.filecount; i++ ) {
        if (args.verbose) {
            fprintf(stderr, "# Processing file %d: %s\n", i, args.filenames[i]);
        }
        int err = pthread_create(&threads[i], NULL, &threaded_find, &payloads[i]);
        if (err) {
            fprintf(stderr, "%s\n", strerror(err));
            exit(EXIT_FAILURE);
        }
    }

    for (int i=0; i<args.filecount; i++) {
        pthread_join(threads[i], NULL);
    }

    if (args.timing) {
        fprintf(stderr, "#COMMAND");
        for (int i = 0; i < argc; i++) {
            fprintf(stderr, " %s", argv[i]);
        }
        fprintf(stderr, "\n");

        int ok_count = 0;
        for (int i = 0; i < args.filecount; i++) {
            if (file_ok[i]) ok_count++;
        }

        if (args.filecount == 0 || ok_count == 0) {
            fprintf(stderr, "#TIMING_SUMMARY algorithm=%s files=%d errors=%d bytes=0 min=0 avg=0 max=0\n",
                    args.algorithm_code, args.filecount, args.filecount - ok_count);
        } else {
            long min = 0;
            long max = 0;
            long total = 0;
            unsigned long total_bytes = 0;
            int first = 1;
            for (int i = 0; i < args.filecount; i++) {
                if (!file_ok[i]) continue;
                if (first) {
                    min = elapsed_usec[i];
                    max = elapsed_usec[i];
                    first = 0;
                }
                if (elapsed_usec[i] < min) min = elapsed_usec[i];
                if (elapsed_usec[i] > max) max = elapsed_usec[i];
                total += elapsed_usec[i];
                total_bytes += bytes_searched[i];
            }
            long avg = total / ok_count;

            fprintf(stderr, "#TIMING_SUMMARY algorithm=%s files=%d errors=%d bytes=%lu min=%ld avg=%ld max=%ld\n",
                    args.algorithm_code, args.filecount, args.filecount - ok_count, total_bytes, min, avg, max);
        }
    }

    exit(EXIT_SUCCESS);
}
