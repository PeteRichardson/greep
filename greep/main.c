//
//  main.c
//  greep
//
//  Created by Peter Richardson on 12/3/21.
//

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "options.h"

const char *argp_program_version     = "greep 0.2";
const char *argp_program_bug_address = "<pete@peterichardson.com>";

void find(char *search_word,
          FILE *stream,
          char *filename,
          void (*found_callback)(char *, unsigned long, char *)) {
    unsigned long search_word_maxindex;
    
    search_word_maxindex = strlen(search_word) - 1;

    char c = '\0';
    unsigned long i = 0;
    unsigned long lineno = 1;
    while (1) {
        c = fgetc(stream);
        if (c == '\n') {
            lineno++;
        }
//        printf("%4lu\t%c\n",i, c);
        if (c == EOF) {
            break;
        }
        if (i != 0 && c != search_word[i]) {
            i = 0;
            ungetc(c, stream);
        }
        if (c == search_word[i]) {
            if (i==search_word_maxindex ) {
                i = 0;
                (*found_callback)(filename, lineno, search_word);
            }
            i++;
        }
    }
}

void found_callback(char *filename, unsigned long line_num, char *search_word) {
    printf("%s:%lu %s\n", filename, line_num, search_word);
}

int main(int argc, char *argv[]) {
    struct arguments arguments = {
        .verbose = 0,
        .search_word = NULL,
        .stream = 0
    };
    int err = argp_parse (&argp, argc, argv, 0, 0, &arguments);
    if (err != 0) {
        fprintf(stderr, "unabled to parse command line options: %d", err);
        exit(EXIT_FAILURE);
    }
    
    if (arguments.verbose) {
        fprintf(stderr, "# Searching for '%s'\n", arguments.search_word);
        fprintf(stderr, "# Processing %d files: %s...%s\n", arguments.filecount, arguments.filenames[0], arguments.filenames[arguments.filecount - 1]);
    }
    
    FILE *stream;
    char *filename;
    for (int i = 0; i < arguments.filecount; i++ ) {
        filename = arguments.filenames[i];
        stream = fopen(filename, "r");
        if (stream == NULL) {
            fprintf(stderr, "# ERROR: Unable to open file '%s' for reading.\n", filename);
            continue;
        }
        find(arguments.search_word, stream, filename, found_callback);
    }

    exit(EXIT_SUCCESS);
}
