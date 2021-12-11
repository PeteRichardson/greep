//
//  options.h
//  greep
//
//  Created by Peter Richardson on 12/11/21.
//

#ifndef options_h
#define options_h

#include <argp.h>


struct arguments {
//    char *args[2];                /* arg1 & arg2 */
    int  verbose;
    char *search_word;
    char **filenames;
    int  filecount;
    FILE *stream;
};

static char doc[] = "greep -- a homegrown (and less functional) grep";

/* A description of the arguments we accept. */
static char args_doc[] = "FILES...";

/* The options we understand. */
static struct argp_option options[] = {
    {"verbose",     'v', "verbose",     OPTION_ARG_OPTIONAL, "Produce verbose output" },
    {"search_word", 's', "search_word", 0, "String to search for" },
    { 0 }
};

/* Parse a single option. */
error_t parse_opt (int key, char *arg, struct argp_state *state);

static struct argp argp = { options, parse_opt, args_doc, doc };

#endif /* options_h */