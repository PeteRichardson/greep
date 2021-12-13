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
#include <unistd.h>

#include "options.h"

const char *argp_program_version     = "greep 0.6";
const char *argp_program_bug_address = "<pete@peterichardson.com>";


typedef void (*callback_t) (const char *, unsigned long, const char *);

typedef void (*search_alg_t) (const char *,         // search_word
                              const char *,         // filename
                              const char *,         // start
                              const unsigned long,  // length
                              callback_t *);        // callback


typedef struct threadargs_t {
    const char *search_word;
    const char *filename;
    search_alg_t search_alg;
    callback_t found_callback;
} threadargs_t;

/*
 Find using brute force algorithm
    assume file is in memory
*/
void find_bf(const char *search_word,
             const char *filename,
             const char *start,
             const unsigned long length,
             callback_t *found_callback) {
    // mmap file
    FILE *stream = fopen(filename, "r");
    if (stream == NULL) {
        fprintf(stderr, "# ERROR: Unable to open file '%s' for reading.\n", filename);
        return;
    }

    unsigned long search_word_maxindex = strlen(search_word) - 1;

    char c = '\0';
    unsigned long i = 0;
    unsigned long lineno = 1;
    while ((c = fgetc(stream)) != EOF) {
        //  printf("%s: %c, %lu\n", filename, c, i);
        if (c == '\n') lineno++;
        
        if (i != 0 && c != search_word[i]) {
            i = 0;
            ungetc(c, stream);
        } else if (c == search_word[i]) {
            if (i==search_word_maxindex ) {
                i = 0;
                (*found_callback)(filename, lineno, search_word);
                // TODO: print the whole line.  for now print search_word.
            }
            i++;
        }
    }
    
    fclose(stream);

}

void *threaded_find(void *arg) {
    threadargs_t thread_args = *(threadargs_t *) arg;
    
    // mmap file
    (thread_args.search_alg)(thread_args.search_word, thread_args.filename, 0, 0, &thread_args.found_callback);
    // munmap file

    return 0;
}


// Run this code whenever a match is found
void found_callback(const char *filename, unsigned long line_num, const char *line) {
    printf("%s:%lu %s\n", filename, line_num, line);
}

int main(int argc, char *argv[]) {
    arguments_t args = ARGUMENT_DEFAULT_VALUES;
    int err = argp_parse (&argp, argc, argv, 0, 0, &args);
    if (err != 0) {
        fprintf(stderr, "# ERROR: Unable to parse command line options: %d", err);
        exit(EXIT_FAILURE);
    }
    
    if (args.verbose) {
        fprintf(stderr, "# Searching for '%s'\n", args.search_word);
    }
    
    pthread_t threads[args.filecount];
    threadargs_t payloads[args.filecount];
    for (int i = 0; i < args.filecount; i++ ) {
        threadargs_t thread_args = {
            .search_word = args.search_word,
            .filename = args.filenames[i],
            .search_alg = find_bf,
            .found_callback = found_callback
        };
        payloads[i] = thread_args;
    }
        
    for (int i = 0; i < args.filecount; i++ ) {
        if (args.verbose) {
            fprintf(stderr, "# Processing file %d: %s\n", i, args.filenames[i]);
        }
        int err = pthread_create(&threads[i], NULL, &threaded_find, &payloads[i]);
        if (err) {
            printf("%s\n", strerror(err), stderr);
            exit(EXIT_FAILURE);
        }
    }

    for (int i=0; i<args.filecount; i++) {
        pthread_join(threads[i], NULL);
    }
    
    exit(EXIT_SUCCESS);
}
