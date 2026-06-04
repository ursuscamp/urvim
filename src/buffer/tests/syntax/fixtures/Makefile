# Makefile syntax fixture
# Comment line

all: build test

NAME := urvim
VERSION := 1.0.0
CC := gcc
CFLAGS := -Wall -Wextra -O2
LDFLAGS := -lm

include common.mk

SRCS := $(wildcard src/*.c)
OBJS := $(SRCS:.c=.o)

.PHONY: all build test clean install

build: $(OBJS)
	$(CC) $(CFLAGS) -o $(NAME) $(OBJS) $(LDFLAGS)

src/%.o: src/%.c $(wildcard src/*.h)
	$(CC) $(CFLAGS) -c $< -o $@

test: build
	./$(NAME) --test

clean:
	rm -f $(OBJS) $(NAME)

install: build
	cp $(NAME) /usr/local/bin/$(NAME)
	cp $(NAME).1 /usr/local/share/man/man1/

docs:
	pandoc README.md -o docs/$(NAME).html
	pandoc README.md -o docs/$(NAME).pdf

ifeq ($(DEBUG),1)
CFLAGS += -g -DDEBUG
endif

ifneq ($(shell uname),Darwin)
LDFLAGS += -lrt
endif
