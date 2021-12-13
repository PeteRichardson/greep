//
//  search_default.c
//  greep
//
//  Created by Peter Richardson on 12/13/21.
//
#include <stdlib.h>
#include <stdio.h>
#include <strings.h>

#include "search_algorithms.h"

/*
 Find using brute force algorithm
    assume file is in memory
*/
void find_bf (const char *search_word,
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
