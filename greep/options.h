//
//  options.h
//  greep
//
//  Created by Peter Richardson on 12/11/21.
//

#ifndef options_h
#define options_h

typedef struct arguments_t {
    int  verbose;
    char *search_word;
    char **filenames;
    int  filecount;
} arguments_t;

static char *default_input_files[] = { "/dev/stdin" };

static arguments_t ARGUMENT_DEFAULT_VALUES = {
    .verbose = 0,
    .search_word = NULL,
    .filecount = 1,
    .filenames = (char**)default_input_files
};

void parse_args(int argc, char *argv[], arguments_t *args);

#endif /* options_h */
