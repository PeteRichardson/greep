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

const char *argp_program_version     = "greep 0.5";
const char *argp_program_bug_address = "<pete@peterichardson.com>";

typedef struct threadargs_t {
    const char *search_word;
    const char *filename;
    void (*found_callback)(const char *, unsigned long, const char *);
} threadargs_t;

/*
 Find a word in a stream of chars.
 
 TODO: memory map the file.  Maybe faster?
 TODO: use better string search algorithm.
        see http://www-igm.univ-mlv.fr/~lecroq/string/index.html
 */
void find(const char *search_word,
          const char *filename,
          void (*found_callback)(const char *, unsigned long, const char *)) {
    
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
    find(thread_args.search_word, thread_args.filename, thread_args.found_callback);
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
