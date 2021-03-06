//
//  search_default.c
//  greep
//
//  Created by Peter Richardson on 12/13/21.
//

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

    unsigned long search_word_maxindex = strlen(search_word) - 1;
    unsigned long i = 0;
    unsigned long lineno = 1;
    
    char *line_start = (char *)start;
    char *line_end = NULL;

    char *c = (char *)start;
    char *end = (char *)start + length;
    while (c < end) {
        //printf("%s: %c, %lu\n", filename, *c, i);
        if (*c == '\n') {
            i = 0;
            line_start = c + 1;
            lineno++;
        }
        
        if (i != 0 && *c != search_word[i]) {
            i = 0;
            c--;
        } else if (*c == search_word[i]) {
            if (i==search_word_maxindex ) {
                i = 0;
                // move to end of line (or EOF)
                for ( ; *c != '\n' && *c != EOF; c++ ) {
                    // do nothing
                };

                line_end = c;
                (*found_callback)(filename, lineno, line_start, line_end);
                // TODO: might as well skip to the next line, since we've already flagged this one.
                // TODO: print the whole line.  for now print search_word.
            }
            i++;
        }
        c++;
    }
}
