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
#include "search_algorithms/search_algorithms.h"

static char usage[] =
    "usage: greep [-v] [-a ALGORITHM] STRING [FILES...]\n"
    "       greep -l\n";

static struct option long_options[] = {
    {"verbose",   no_argument,       0, 'v'},
    {"algorithm", required_argument, 0, 'a'},
    {"list",      no_argument,       0, 'l'},
    {0, 0, 0, 0}
};

void parse_args(int argc, char *argv[], arguments_t *args)
{
    int opt;
    while ((opt = getopt_long(argc, argv, "va:l", long_options, NULL)) != -1) {
        switch (opt) {
            case 'v':
                args->verbose = 1;
                break;

            case 'a':
                args->algorithm_code = optarg;
                break;

            case 'l':
                list_algorithms(stdout);
                exit(EXIT_SUCCESS);

            default:
                fputs(usage, stderr);
                exit(EXIT_FAILURE);
        }
    }

    if (find_algorithm(args->algorithm_code) == NULL) {
        fprintf(stderr, "# ERROR: unknown algorithm '%s'. Run 'greep -l' to see available algorithms.\n", args->algorithm_code);
        exit(EXIT_FAILURE);
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
