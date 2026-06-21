//
//  filelist.h
//  greep
//

#ifndef filelist_h
#define filelist_h

char **read_filelist(const char *path, int *out_count);
char **expand_paths(char **paths, int count, int *out_count);

#endif /* filelist_h */
