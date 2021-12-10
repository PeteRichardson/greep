//
//  main.c
//  greep
//
//  Created by Peter Richardson on 12/3/21.
//

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SEP_CHARS "/\\"

// Returns file leaf name from abs path
const char *leaf_name(const char *path) {
    char *token = strtok(path, SEP_CHARS);
    char *prev_token = "greep";
    while (token != NULL) {
        prev_token = token;
        token = strtok(NULL, SEP_CHARS);  // this will be null after the last '/'
    }
    return prev_token;
}

int main(int argc, const char *argv[]) {
    const char *search_word;
    unsigned long search_word_maxindex;
    
    if (argc != 2) {
        const char *exe_name = leaf_name(argv[0]);
        printf("usage: %s <string>\n", exe_name);
        exit(EXIT_FAILURE);
        
    }
    search_word = argv[1];
    search_word_maxindex = strlen(search_word) - 1;
    printf("# Searching for %s\n", search_word);
    //printf("# First char: %c\n", search_word[0]);
    
    char c = '\0';
    unsigned long i = 0;
    unsigned long lineno = 1;
    while (1) {
        c = getchar();
        if (c == '\n') {
            lineno++;
        }
        //printf("%4lu\t%c\n",i, c);
        if (c == EOF) {
            break;
        }
        if (i != 0 && c != search_word[i]) {
            i = 0;
            ungetc(c, stdin);
            //printf("[nope]\n");
            continue;
        }
        if (c == search_word[i]) {
            if (i==search_word_maxindex ) {
                printf("# Found it: %5lu: %s\n", lineno, search_word);
                i = 0;
                continue;
            }
            i++;
        }
        
    
    }
    exit(EXIT_SUCCESS);
}
