//
//  options.c
//  greep
//
//  Created by Peter Richardson on 12/11/21.
//

#include "options.h"
#include <stdlib.h>

#include <string.h>

#define SEP_CHARS "//"

// Returns file leaf name from abs path
const char *leaf_name(char *path) {
    char *token = strtok(path, SEP_CHARS);
    char *prev_token = "greep";
    while (token != NULL) {
        prev_token = token;
        token = strtok(NULL, SEP_CHARS);  // this will be null after the last '/'
    }
    return prev_token;
}

error_t parse_opt (int key, char *arg, struct argp_state *state)
{
    /* Get the input argument from argp_parse, which we
     know is a pointer to our arguments structure. */
    struct arguments *arguments = state->input;
    //fprintf(stderr, "arg: %s\n", arg);

    switch (key) {
        case 'v':
            //fprintf(stderr, "\t%d. v: %s\n", state->arg_num, arg);
            arguments->verbose = 1;
            break;
            
        case 's':
            //fprintf(stderr, "\t%d. s: %s\n", state->arg_num, arg);
            arguments->search_word = arg;
            break;
 
        case ARGP_KEY_ARG:
            return ARGP_ERR_UNKNOWN;
            break;

        case ARGP_KEY_ARGS:
            arguments->filenames = state->argv + state->next;
            arguments->filecount = state->argc - state->next;
            break;

        default:
            return ARGP_ERR_UNKNOWN;
    }
    return 0;
}
