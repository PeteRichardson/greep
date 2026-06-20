//
//  search_bmh.c
//  greep
//

#include <string.h>

#include "search_algorithms.h"

/*
 Find using the Boyer-Moore-Horspool algorithm.
 Assumes file is in memory. Reports at most one match per line (the line
 containing the first occurrence of search_word), matching the per-line
 reporting behavior of find_bf.
*/
void find_bmh(const char *search_word,
              const char *filename,
              const char *start,
              const unsigned long length,
              callback_t *found_callback) {

    unsigned long word_len = strlen(search_word);
    if (word_len == 0 || length < word_len) {
        return;
    }

    unsigned long shift[256];
    for (int i = 0; i < 256; i++) {
        shift[i] = word_len;
    }
    for (unsigned long i = 0; i < word_len - 1; i++) {
        shift[(unsigned char) search_word[i]] = word_len - 1 - i;
    }

    const char *end = start + length;
    unsigned long lineno = 1;
    const char *line_start = start;
    const char *scan_pos = start;

    const char *window = start;
    while (window + word_len <= end) {
        // Advance line tracking up to the current window position. scan_pos
        // only ever moves forward, so this stays O(n) total across the run
        // even though window can jump ahead by more than one byte per step.
        for (; scan_pos < window; scan_pos++) {
            if (*scan_pos == '\n') {
                lineno++;
                line_start = scan_pos + 1;
            }
        }

        long j = (long) word_len - 1;
        while (j >= 0 && window[j] == search_word[j]) {
            j--;
        }

        if (j < 0) {
            const char *line_end = window + word_len;
            while (line_end < end && *line_end != '\n') {
                line_end++;
            }
            (*found_callback)(filename, lineno, line_start, line_end);

            window = (line_end < end) ? line_end + 1 : end;
            scan_pos = window;
            lineno++;
            line_start = window;
        } else {
            unsigned char bad_char = (unsigned char) window[word_len - 1];
            window += shift[bad_char];
        }
    }
}
