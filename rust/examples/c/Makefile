# Build the C example against the library cargo produces.
#
#     make -C rust/examples/c run          # shared
#     make -C rust/examples/c run-static   # static
#
# There is nothing to configure: both targets build the library first.

CARGO ?= cargo
CFLAGS ?= -O2 -Wall -Wextra -Werror -std=c11
ROOT := ../..
INCLUDE := $(ROOT)/include
LIBDIR := $(ROOT)/target/release

.PHONY: all run run-static clean lib

all: demo

lib:
	$(CARGO) build --release --manifest-path $(ROOT)/Cargo.toml

demo: demo.c lib
	$(CC) $(CFLAGS) -I$(INCLUDE) demo.c -L$(LIBDIR) -lbase91z -o demo

# Statically linked, which is how the ABI gets checked against the archive as
# well as the shared object. The Rust runtime needs these three on Linux.
demo-static: demo.c lib
	$(CC) $(CFLAGS) -I$(INCLUDE) demo.c $(LIBDIR)/libbase91z.a -lm -lpthread -ldl -o demo-static

run: demo
	LD_LIBRARY_PATH=$(LIBDIR) ./demo

run-static: demo-static
	./demo-static

clean:
	rm -f demo demo-static
