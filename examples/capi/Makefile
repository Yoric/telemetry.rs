CC ?= gcc
CFLAGS += -I../../capi/include/
LDFLAGS += -L../../capi/target/debug/ -ltelemetry_capi -lc -lm -lpthread -ldl
OS := $(shell uname -s)
ifeq ($(OS),Linux)
LDFLAGS += -lrt
endif

DEPS := ../../capi/include/telemetry.h ../../capi/target/debug/libtelemetry_capi.a Makefile
OBJ := telemetry.o

all: telemetry
	./telemetry

%.o: %.c $(DEPS)
	$(CC) -c -o $@ $< $(CFLAGS)

telemetry: $(OBJ)
	$(CC) -o $@ $^ $(LDFLAGS)
