# Steampunk

[![CI](https://github.com/bl-nero/steampunk/actions/workflows/ci.yml/badge.svg)](https://github.com/bl-nero/steampunk/actions/workflows/ci.yml)

Because what the world needs right now is yet another retro computing emulator.
So yeah, here it is. This started as a "father and son" hobby project. It's
built with no particular agenda other than having fun (and learning Rust).
Currently, the project contains an Atari 2600 emulator based on a cycle-based
6502 implementation, as well as some humble beginnings of a Commodore 64
emulator.

# Installing requirements

## Rust

The emulator is built in Rust, so obviously, first you need to
[install the Rust toolchain](https://www.rust-lang.org/tools/install). At the
moment of writing this document, a nightly version of Rust is required:

```sh
rustup install nightly
rustup default nightly
```

## Development libraries

Depending on your system, you may need to install the following libraries to
build Steampunk:

- libasound2-dev
- libsdl2-dev

## cc65

One important dependency is a [cc65 compiler](https://cc65.github.io/).
Technically, we only rely on its 6502 assembler, but it comes in a bigger
package. We also only use it for tests, so it could be probably skipped for a
regular build, but we are lazy.

- **On Mac,** it's enough to say `brew install cc65`, provided that you already
  have [Homebrew](https://brew.sh/) installed.
- **On Windows,** it's a bit more involved, unsurprisingly. You first need to
  download and unpack the
  [Windows snapshot of cc65](https://sourceforge.net/projects/cc65/files/cc65-snapshot-win32.zip)
  to a directory of your choice. Next, you need to add the `bin` directory of
  cc65 to the system `PATH` variable.
  [Here is a nice tutorial](https://www.howtogeek.com/118594/how-to-edit-your-system-path-for-easy-command-line-access/)
  if you don't know how to do it.
- **On Linux**, refer to
  [this page](https://software.opensuse.org/download.html?project=home%3Astrik&package=cc65).
  Note that the latest Debian packages should work on Ubuntu (they are used in
  this project's CI build).

# Atari 2600 emulator

## Building and running

Assuming that both Rust and cc65 are properly installed, simply run the
following command:

```sh
cargo run --release --bin=atari2600 -- <rom-file-path>
```

Where `<rom-file-path>` is a path of the Atari 2600 ROM to be executed. Make
sure to run the optimized binary (`--release`); the debug one is way too slow.

## Keyboard mapping

- **1**: Toggle TV type switch
- **2**: Toggle player 1 difficulty
- **3**: Toggle player 2 difficulty
- **4**: Game select
- **5**: Game reset
- **W**, **A**, **S**, **D**, **Left Shift**, **Space**: Player 1 Joystick
- **I**, **J**, **K**, **L**, **N**, **.**, arrow keys: Player 2 Joystick

## Compatibility

Currently, the following official Atari 2600 cartridges are known to be
supported:

- _Basic Math_, a.k.a. _Fun with Numbers_
- _Combat_
- _Air-Sea Battle_
- _Starship_… sort of. For some reason, we are unable to aim down.
- _Surround_

# Commodore 64 emulator

The C64 emulator is so far capable of running simple BASIC programs and loading
stuff from TAP files, but it's still in a very early stage of development. To
run it, use a following command:

```sh
cargo run --bin=c64 --release
```

As in the Atari 2600 case, it's important to use the release build. Actually,
it's even more important; since C64 is more complicated, the debug build crawls
like a stoned snail.

In order to use a TAP file, you need to specify its path while starting the
emulator (no tape switching supported just yet):

```sh
cargo run --bin=c64 --release -- --tape=<tape_path>
```

Then, while the emulator is running, press **⌘P** (or **⊞P**, depending on the
system) to press Play.

# Debugging

One nice feature that helps development is ability to attach VS Code debugger to
debug the emulated 6502 code. If you ever wanted to use a 2020s toolchain to
debug 1970s technology, well, look no further. Using the
[Steampunk 6502 debugger extension](https://marketplace.visualstudio.com/items?itemName=BartoszLeper.steampunk-6502-debugger),
you can debug 6502 assembly code on both Atari 2600 and C64. Please refer to the
debugger extension's documentation for detailed usage instructions.

Note that it's still recommended to use a release build of Steampunk for 6502
debugging; this feature doesn't depend on debugging the emulator code itself.

# Known issues and limitations

- Unofficial 6502 opcodes are not supported
- No support for bank switching (Atari 2600)
- No support for input devices other than joysticks (Atari 2600)
- Can't press the Stop button on Datasette just yet. YOLO.
