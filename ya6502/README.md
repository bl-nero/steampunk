# YA6502 — Yet Another 6502 CPU emulator

YA6502 is a straightforward implementation of a 6502 CPU emulator, built as a playground for learning Rust and to be used in an Atari 2600 emulator. This package is work in progress; not all 6502 features are supported at the moment.

# Getting started

To use the 6502 CPU in your project, you need to provide an implementation of the `Memory` trait that represents an address space of your emulated hardware. To get started quickly, you can just use `ya6502::memory::Ram`. Having that, instantiate your CPU and make it run:

```
use ya6502::Cpu;
use ya6502::memory::Ram;

let memory = Box::new(Ram::new(16)); // 2^16 bytes

// (Populate the memory here.)

let cpu = Cpu::new(memory);
cpu.reset();
loop {
    cpu.tick()?;
}
```

That's it!