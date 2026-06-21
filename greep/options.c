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
#include "filelist.h"
#include "search_algorithms/search_algorithms.h"

static char usage[] =
    "usage: greep [-v] [-a ALGORITHM] [-f FILELIST] STRING [FILES...]\n"
    "       greep -l\n";

static struct option long_options[] = {
    {"verbose",   no_argument,       0, 'v'},
    {"algorithm", required_argument, 0, 'a'},
    {"list",      no_argument,       0, 'l'},
    // TODO: --dump-filelist <path> to capture the resolved file set for reuse
    {"filelist",  required_argument, 0, 'f'},
    {0, 0, 0, 0}
};

void parse_args(int argc, char *argv[], arguments_t *args)
{
    int opt;
    while ((opt = getopt_long(argc, argv, "va:lf:", long_options, NULL)) != -1) {
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

            case 'f':
                args->filelist_path = optarg;
                break;

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

    int positional_filecount = argc - optind - 1;
    if (args->filelist_path != NULL && positional_filecount > 0) {
        fprintf(stderr, "# ERROR: cannot combine -f/--filelist with positional file arguments\n");
        exit(EXIT_FAILURE);
    }

    char **initial_filenames;
    int initial_filecount;

    if (args->filelist_path != NULL) {
        initial_filenames = read_filelist(args->filelist_path, &initial_filecount);
    } else if (positional_filecount > 0) {
        initial_filenames = &argv[optind + 1];
        initial_filecount = positional_filecount;
    } else {
        initial_filenames = args->filenames;
        initial_filecount = args->filecount;
    }

    args->filenames = expand_paths(initial_filenames, initial_filecount, &args->filecount);
}
