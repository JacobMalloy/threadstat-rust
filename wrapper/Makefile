CC ?= clang
CFLAGS ?= -O3

wrapper.so: wrapper.c
	$(CC) $(CFLAGS) wrapper.c -shared -fpic -o wrapper.so -lrt -lpthread -ldl

clean:
	rm -f wrapper.so

.PHONY: clean
