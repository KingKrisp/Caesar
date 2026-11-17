# Build

Start by cloning the repository:

```sh
git clone https://github.com/baskerville/plato.git
cd plato
```

## Plato

#### Preliminary

Install the appropriate [compiler toolchain](https://drive.google.com/drive/folders/1YT6x2X070-cg_E8iWvNUUrWg5-t_YcV0) (the binaries of the `bin` directory need to be in your path).

Install the required dependencies: `wget`, `curl`, `git`, `pkg-config`, `unzip`, `jq`, `patchelf`.

Install *rustup*:
```sh
curl https://sh.rustup.rs -sSf | sh
```

Install the appropriate target:
```sh
rustup target add arm-unknown-linux-gnueabihf
```

### Build Phase

```sh
./build.sh
```

### Distribution

```sh
./dist.sh
```

## Developer Tools

Install the required dependencies: *MuPDF 1.27.0*, *DjVuLibre*, *FreeType*, *HarfBuzz*.

### Emulator

Install one additional dependency: *SDL2*.

You can then run the emulator with:
```sh
./run-emulator.sh
```

### Importer

You can install the importer with:
```sh
./install-importer.sh
```

## Building on an Apple Silicon Mac

The Clara HD build was validated with Rust 1.100.0, LLVM/LLD 22 and the official
Kobo/Linaro 4.9.4-2017.01 ARM sysroot. The Linux compiler executable from that
archive is not run on macOS; Clang uses its ARM headers and libraries.

```sh
brew install llvm@22 lld@22 rustup
export PATH="$(brew --prefix rustup)/bin:$PATH"
rustup toolchain install stable --profile minimal
rustup target add arm-unknown-linux-gnueabihf
```

Download and extract the official
[Linaro toolchain archive](https://developer.arm.com/-/cdn-downloads/permalink/legacy-linaro-gnu-toolchains/4.9-2017.01/gcc-linaro-4.9.4-2017.01-x86_64_arm-linux-gnueabihf.tar.xz).
Its SHA-256 must be
`22914118fd963f953824b58107015c6953b5bbdccbdcf25ad9fd9a2f9f11ac07`.

Extract the `libs/`, `bin/`, `resources/` and `hyphenation-patterns/` directories
from the official [Plato 0.9.45 runtime archive](https://github.com/baskerville/plato/releases/download/0.9.45/plato-0.9.45.zip)
into this checkout. Extract the [MuPDF 1.27.0 source archive](https://casper.mupdf.com/downloads/archive/mupdf-1.27.0-source.tar.gz)
into `thirdparty/mupdf/` so `thirdparty/mupdf/include/mupdf/fitz.h` exists.
Then run from the repository root:

```sh
python3 scripts/build-kobo-macos.py --toolchain /path/to/extracted/linaro --tests
```

This builds `target/arm-unknown-linux-gnueabihf/release/plato` and compiles the ARM
test executables. Executing those tests requires an ARM Linux runtime or QEMU;
`--tests` alone does not run them.

`contrib/kobo-lld-layout.ld` keeps `.rel.dyn` and `.rel.plt` adjacent. Without it,
LLD places ARM unwind information between the two tables and the old loader
segfaults before `main`. The script also prepares a separate link-only copy of
MuPDF with corrected symbol bindings for LLD. The runtime libraries in `libs/`
remain untouched and are the ones to distribute to the reader.
