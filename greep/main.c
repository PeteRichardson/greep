//
//  main.c
//  greep
//
//  Created by Peter Richardson on 12/3/21.
//

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syslimits.h>
#include <fcntl.h>

#define SEP_CHARS "//"

typedef struct OPTIONS {
    char *search_word;
    FILE *stream;       // default to 0 (stdin)
} OPTIONS;


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


void find(char *search_word, FILE *stream, void (*found_callback)(unsigned long line, char *search_word)) {
    unsigned long search_word_maxindex;
    
    search_word_maxindex = strlen(search_word) - 1;

    char c = '\0';
    unsigned long i = 0;
    unsigned long lineno = 1;
    while (1) {
        c = fgetc(stream);
        if (c == '\n') {
            lineno++;
        }
//        printf("%4lu\t%c\n",i, c);
        if (c == EOF) {
            break;
        }
        if (i != 0 && c != search_word[i]) {
            i = 0;
            ungetc(c, stdin);
//            printf("[nope]\n");
            continue;
        }
        if (c == search_word[i]) {
            if (i==search_word_maxindex ) {
                (*found_callback)(lineno, search_word);
                i = 0;
                continue;
            }
            i++;
        }
    }
}

void found_callback(unsigned long line_num, char *search_word) {
    printf("%3lu: %s\n", line_num, search_word);
}


void parse_options(OPTIONS *options, int argc, char *argv[]) {
    char *stream_name;
    options->stream = 0;
    switch(argc) {
        case 3 :
            stream_name = argv[2];
        case 2 :
            stream_name = "/dev/stdin";
             break;
 
        default :
            printf("usage: %s <string> [file]\n", leaf_name(argv[0]));
            exit(EXIT_FAILURE);
    }
    options->stream = fopen(stream_name, "r");
    if (options->stream == NULL) {
        fprintf(stderr, "Unable to open file '%s' for reading.\n", argv[2]);
        exit(EXIT_FAILURE);
    }
    options->search_word = argv[1];
    
    printf("# Searching for '%s'\n", options->search_word);
    printf("# Searching in '%s'\n", stream_name);

}

int main(int argc, char *argv[]) {
    // usage:  greep <string_to_find> [file_to_search]
    OPTIONS options;
    parse_options(&options, argc, argv);
    
    find(options.search_word, options.stream, found_callback);
    
    exit(EXIT_SUCCESS);
}
