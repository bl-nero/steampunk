# Atari 2600 emulator

This is a "father and son" hobby project: an Atari 2600 emulator built with no particular agenda other than having fun.

# Building and running

## Installing requirements

### Rust

The emulator is built in Rust, so obviously, first you need to [install the Rust toolchain](https://www.rust-lang.org/tools/install). At the moment of writing this document, a nightly version of Rust is required:

```sh
rustup install nightly
rustup default nightly
```

### cc65

The second dependency is a [cc65 compiler](https://cc65.github.io/). Technically, we only rely on its 6502 assembler, but it comes in a bigger package. We also only use it for tests, so it could be probably skipped for a regular build, but we are lazy.

* **On Mac,** it's enough to say `brew install cc65`, provided that you already have [Homebrew](https://brew.sh/) installed.
* **On Windows,** it's a bit more involved, unsurprisingly. You first need to download and unpack the [Windows snapshot of cc65](https://sourceforge.net/projects/cc65/files/cc65-snapshot-win32.zip) to a directory of your choice. Next, you need to add the `bin` directory of cc65 to the system `PATH` variable. [Here is a nice tutorial](https://www.howtogeek.com/118594/how-to-edit-your-system-path-for-easy-command-line-access/) if you don't know how to do it.

## Building and running the emulator

Assuming that both Rust and cc65 are properly installed, simply run the following command:

```
cargo run --release -- <rom-file-path>
```

Where `<rom-file-path>` is a path of the Atari 2600 ROM to be executed.

# Compatibility

In its current state, the emulator only supports a handful of opcodes and a couple of TIA registers. It's not yet capable of executing even a single full game, but it's still capable of generating a couple of cool effects! Having built your emulator (either through `cargo run`, `cargo test` or `cargo build`), poke around the `target/debug/build/atari-emulator-<hash>/out/roms` directory. The `<hash>` component is unstable, so I can't give it here. Inside, you should find a couple of working ROMs. These are built from sources in the `src/asm` directory.
