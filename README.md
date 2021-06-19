# Atari 2600 emulator

Because what the world needs right now is yet another Atari 2600 emulator. So yeah, here it is. This started as a "father and son" hobby project. It's built with no particular agenda other than having fun (and learning Rust).

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

In its current state, the emulator is just enough to support at least one officially supported Atari 2600 cartridge — I'm avoiding the word “game” purposefully here — _Basic Math_, a.k.a. _Fun with Numbers_. TIA supports playfield graphics, joystick readouts and nothing more, which is exactly what _Basic Math_ requires. No player graphics yet, and no sound. The 6502 module also supports just enough to support that single cartridge as a starting point, so it only supports only 111 out of 151 official 65xx opcodes.
