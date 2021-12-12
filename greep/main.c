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

const char *argp_program_version     = "greep 0.4";
const char *argp_program_bug_address = "<pete@peterichardson.com>";

/*
 Find a word in a stream of chars.
 
 TODO: memory map the file.  Maybe faster?
 TODO: read stream a line at a time, and output the whole line
 */
void find(char *search_word,
          char *filename,
          void (*found_callback)(char *, unsigned long, char *)) {
    
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
//        printf("%c, %lu\n", c, i);
        if (c == '\n') lineno++;
        
        if (i != 0 && c != search_word[i]) {
            i = 0;
            ungetc(c, stream);
        } else if (c == search_word[i]) {
            if (i==search_word_maxindex ) {
                i = 0;
                (*found_callback)(filename, lineno, search_word);
            }
            i++;
        }
    }
    fclose(stream);
}

// Run this code whenever a match is found
void found_callback(char *filename, unsigned long line_num, char *line) {
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
        fprintf(stderr, "# Processing %d files: %s...%s\n", args.filecount, args.filenames[0], args.filenames[args.filecount - 1]);
    }
    
    for (int i = 0; i < args.filecount; i++ ) {
         find(args.search_word, args.filenames[i], found_callback);
    }

    exit(EXIT_SUCCESS);
}
