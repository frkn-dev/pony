# Makefile

TARGET = x86_64-unknown-linux-gnu

.PHONY: build

build:
	@echo -n "white" | nc -4u -w0 localhost 1738
	@cross build --target $(TARGET) --release
	@if [ $$? -eq 0 ]; then \
		echo -n "green" | nc -4u -w0 localhost 1738; \
	else \
		echo -n "red" | nc -4u -w0 localhost 1738; \
	fi
