TARGET := greep
SRC_DIR := greep
BUILD_DIR := build

SRCS := $(SRC_DIR)/main.c $(SRC_DIR)/options.c $(SRC_DIR)/search_algorithms/search_default.c
OBJS := $(SRCS:$(SRC_DIR)/%.c=$(BUILD_DIR)/%.o)

ARGP_PREFIX := $(shell brew --prefix argp-standalone)

CC := cc
CFLAGS := -std=gnu11 -Wall -I$(SRC_DIR) -I$(ARGP_PREFIX)/include
LDFLAGS := -L$(ARGP_PREFIX)/lib -largp -lpthread

.PHONY: all clean

all: $(BUILD_DIR)/$(TARGET)

$(BUILD_DIR)/$(TARGET): $(OBJS)
	$(CC) $(OBJS) $(LDFLAGS) -o $@

$(BUILD_DIR)/%.o: $(SRC_DIR)/%.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -rf $(BUILD_DIR)
