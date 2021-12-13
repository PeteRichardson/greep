//
//  options.h
//  greep
//
//  Created by Peter Richardson on 12/11/21.
//

#ifndef options_h
#define options_h

#include <argp.h>


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

static char doc[] = "greep -- a homegrown (and less functional) grep";

/* A description of the arguments we accept. */
static char args_doc[] = "STRING [FILES...]";

/* The options we understand. */
static struct argp_option options[] = {
    {"verbose",     'v', "verbose",     OPTION_ARG_OPTIONAL, "Produce verbose output" },
    { 0 }
};

/* Parse a single option. */
error_t parse_opt (int key, char *arg, struct argp_state *state);

static struct argp argp = { options, parse_opt, args_doc, doc };

#endif /* options_h */
