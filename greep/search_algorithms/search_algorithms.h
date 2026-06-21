//
//  search_algorithms.h
//  greep
//
//  Created by Peter Richardson on 12/13/21.
//
//

#ifndef search_algorithms_h
#define search_algorithms_h

#include <stdio.h>

typedef void (*callback_t) (const char *,  // filename
                            unsigned long,      // line number
                            const char *,       // line start
                            const char *);      // line end

typedef void (*search_alg_t) (const char *,         // search_word
                              const char *,         // filename
                              const char *,         // start
                              const unsigned long,  // length
                              callback_t *);        // callback


//  See various algorithm definitions in http://www-igm.univ-mlv.fr/~lecroq/string/index.html
void find_bf(const char *search_word, const char *filename, const char *start, const unsigned long length, callback_t *found_callback);
void find_bmh(const char *search_word, const char *filename, const char *start, const unsigned long length, callback_t *found_callback);

search_alg_t find_algorithm(const char *code);
void list_algorithms(FILE *out);

#endif /* search_algorithms_h */
