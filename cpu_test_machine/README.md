This is a very simple, minimal CPU test machine with no peripherals. It's meant to execute Klaus Dormann's [functional test suite](https://github.com/Klaus2m5/6502_65C02_functional_tests). It loads given binary (it has to be exactly 64 KiB long), launches it by jumping to $0400 (no reset procedure performed!), and then executes it until it reaches a "trap" (an instruction that loops into itself). Attaching a debugger is also supported.

Note that for licensing reason, the test itself is not included; it needs to be manually downloaded from the [test repository](https://github.com/Klaus2m5/6502_65C02_functional_tests).