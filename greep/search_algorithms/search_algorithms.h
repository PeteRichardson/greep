//
//  search_algorithms.h
//  greep
//
//  Created by Peter Richardson on 12/13/21.
//
//

#ifndef search_algorithms_h
#define search_algorithms_h

typedef void (*callback_t) (const char *, unsigned long, const char *);

typedef void (*search_alg_t) (const char *,         // search_word
                              const char *,         // filename
                              const char *,         // start
                              const unsigned long,  // length
                              callback_t *);        // callback


//  See various algorithm definitions in http://www-igm.univ-mlv.fr/~lecroq/string/index.html
void find_bf(const char *search_word, const char *filename, const char *start, const unsigned long length, callback_t *found_callback);

#endif /* search_algorithms_h */
