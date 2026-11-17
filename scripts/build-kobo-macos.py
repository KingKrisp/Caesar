#!/usr/bin/env python3
"""Build Kobo ARM binaries on an Apple Silicon Mac using the legacy Kobo sysroot.

Requires the upstream 0.9.45 runtime libraries in libs/ and MuPDF 1.27.0 headers
in thirdparty/mupdf/include/. See doc/BUILD.md for preparation and tool versions.
"""
import argparse
import os
from pathlib import Path
import shlex
import shutil
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def run(*args, **kwargs):
    subprocess.run(args, cwd=ROOT, check=True, **kwargs)


def brew_prefix(formula):
    return Path(subprocess.check_output(["brew", "--prefix", formula], text=True).strip())


def elf_sections(data):
    if data[:6] != b"\x7fELF\x01\x01" or struct.unpack_from("<H", data, 18)[0] != 40:
        raise ValueError("Expected a 32-bit little-endian ARM ELF file")
    offset = struct.unpack_from("<I", data, 32)[0]
    size, count, names_index = struct.unpack_from("<HHH", data, 46)
    sections = [struct.unpack_from("<10I", data, offset + i * size) for i in range(count)]
    names = sections[names_index]
    strings = data[names[4]:names[4] + names[5]]
    return sections, {strings[s[0]:strings.index(0, s[0])].decode(): s for s in sections if s[0]}


def prepare_libraries(source, destination):
    # Never alter the runtime libraries that dist.sh will install.
    shutil.copytree(source, destination, dirs_exist_ok=True, symlinks=True)
    for library in destination.glob("*.so.*"):
        alias = destination / (library.name.split(".so.")[0] + ".so")
        if not alias.exists():
            alias.symlink_to(library.name)
    mupdf = destination / "libmupdf.so"
    data = bytearray(mupdf.read_bytes())
    sections, _ = elf_sections(data)
    permitted = {"__bss_start__", "__end__", "__bss_end__", "__bss_start", "_bss_end__", "_end", "_edata"}
    for section in sections:
        if section[1] != 11:  # SHT_DYNSYM
            continue
        strings_section = sections[section[6]]
        strings = data[strings_section[4]:strings_section[4] + strings_section[5]]
        for index in range(section[7], section[5] // section[9]):
            position = section[4] + index * section[9]
            name, _, _, info, _, _ = struct.unpack_from("<IIIBBH", data, position)
            if info >> 4 == 0:
                symbol = strings[name:strings.index(0, name)].decode()
                if symbol not in permitted:
                    raise ValueError(f"Unexpected malformed MuPDF symbol: {symbol}")
                data[position + 12] = 0x10 | (info & 15)
    mupdf.write_bytes(data)


def check_layout(binary):
    _, sections = elf_sections(binary.read_bytes())
    dynamic, plt = sections[".rel.dyn"], sections[".rel.plt"]
    if dynamic[3] + dynamic[5] != plt[3]:
        raise ValueError("Kobo's old loader requires adjacent .rel.dyn and .rel.plt tables")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", required=True, type=Path, help="Extracted official Linaro 4.9.4-2017.01 directory")
    parser.add_argument("--llvm-prefix", type=Path)
    parser.add_argument("--lld-prefix", type=Path)
    parser.add_argument("--tests", action="store_true", help="Also compile the ARM test executables (does not execute them)")
    args = parser.parse_args()
    toolchain = args.toolchain.resolve()
    sysroot = toolchain / "arm-linux-gnueabihf/libc"
    if not (sysroot / "lib/ld-linux-armhf.so.3").exists():
        parser.error("Toolchain does not contain the expected Kobo ARM sysroot")
    llvm = args.llvm_prefix or brew_prefix("llvm@22")
    lld = args.lld_prefix or brew_prefix("lld@22")
    for path in [llvm / "bin/clang", llvm / "bin/llvm-ar", lld / "bin/ld.lld",
                 ROOT / "libs/libmupdf.so", ROOT / "thirdparty/mupdf/include/mupdf/fitz.h"]:
        if not path.exists():
            parser.error(f"Missing prerequisite: {path}")
    target = ROOT / "target/kobo-macos"
    target.mkdir(parents=True, exist_ok=True)
    libraries = target / "link-libs"
    prepare_libraries(ROOT / "libs", libraries)
    linker = target / "arm-linux-gnueabihf-gcc"
    arguments = [str(llvm / "bin/clang"), "--target=arm-linux-gnueabihf", f"--sysroot={sysroot}",
                 f"--gcc-toolchain={toolchain}", f"-fuse-ld={lld / 'bin/ld.lld'}",
                 f"-Wl,-T,{ROOT / 'contrib/kobo-lld-layout.ld'}", "-mcpu=cortex-a9", "-mfpu=neon", "-mfloat-abi=hard"]
    linker.write_text("#!/bin/sh\nexec " + " ".join(map(shlex.quote, arguments)) + ' "$@"\n')
    linker.chmod(0o755)
    env = dict(os.environ, PATH=str(target) + os.pathsep + os.environ["PATH"],
               CAESAR_KOBO_LINK_LIBS=str(libraries), AR_arm_unknown_linux_gnueabihf=str(llvm / "bin/llvm-ar"))
    if not shutil.which("cargo", path=env["PATH"]):
        env["PATH"] = str(brew_prefix("rustup") / "bin") + os.pathsep + env["PATH"]
    wrapper_env = dict(env, CC="arm-linux-gnueabihf-gcc", AR=str(llvm / "bin/llvm-ar"), TARGET_OS="Kobo")
    subprocess.run(["sh", "build.sh"], cwd=ROOT / "mupdf_wrapper", env=wrapper_env, check=True)
    run("cargo", "build", "--locked", "--release", "--target", "arm-unknown-linux-gnueabihf", "-p", "plato", env=env)
    check_layout(ROOT / "target/arm-unknown-linux-gnueabihf/release/plato")
    if args.tests:
        run("cargo", "test", "--locked", "--release", "--target", "arm-unknown-linux-gnueabihf", "-p", "plato-core", "--no-run", env=env)
    print("Kobo ARM build complete; legacy-loader layout verified. Runtime libraries were not modified.")


if __name__ == "__main__":
    main()
