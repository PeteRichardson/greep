//
//  utils.c
//  greep
//
//  Created by Peter Richardson on 12/10/21.
//

#include "utils.h"
#include <string.h>

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
