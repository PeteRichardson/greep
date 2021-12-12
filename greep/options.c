//
//  options.c
//  greep
//
//  Created by Peter Richardson on 12/11/21.
//

#include "options.h"
#include <stdlib.h>

#include <string.h>

error_t parse_opt (int key, char *arg, struct argp_state *state)
{
    struct arguments_t *args = state->input;

    switch (key) {
        case 'v':
            args->verbose = 1;
            break;
            
        case 's':
            args->search_word = arg;
            break;
 
        case ARGP_KEY_ARG:
            return ARGP_ERR_UNKNOWN;
            break;

        case ARGP_KEY_ARGS:
            args->filenames = state->argv + state->next;
            args->filecount = state->argc - state->next;
            break;

        default:
            return ARGP_ERR_UNKNOWN;
    }
    return 0;
}
