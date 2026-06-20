//
//  options.c
//  greep
//
//  Created by Peter Richardson on 12/11/21.
//

#include <stdio.h>
#include <stdlib.h>
#include <getopt.h>

#include "options.h"

static char usage[] = "usage: greep [-v] STRING [FILES...]\n";

static struct option long_options[] = {
    {"verbose", no_argument, 0, 'v'},
    {0, 0, 0, 0}
};

void parse_args(int argc, char *argv[], arguments_t *args)
{
    int opt;
    while ((opt = getopt_long(argc, argv, "v", long_options, NULL)) != -1) {
        switch (opt) {
            case 'v':
                args->verbose = 1;
                break;

            default:
                fputs(usage, stderr);
                exit(EXIT_FAILURE);
        }
    }

    if (optind >= argc) {
        fputs(usage, stderr);
        exit(EXIT_FAILURE);
    }

    args->search_word = argv[optind];

    if (optind + 1 < argc) {
        args->filenames = &argv[optind + 1];
        args->filecount = argc - optind - 1;
    }
}
