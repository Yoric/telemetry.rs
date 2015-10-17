CC ?= gcc
CFLAGS += -I../../capi/include/
LDFLAGS += -L../../capi/target/debug/ -ltelemetry_capi

DEPS := ../../capi/include/telemetry.h
OBJ := telemetry.o

all: telemetry
	./telemetry

%.o: %.c $(DEPS)
	$(CC) -c -o $@ $< $(CFLAGS)

telemetry: $(OBJ)
	$(CC) -o $@ $^ $(LDFLAGS)
