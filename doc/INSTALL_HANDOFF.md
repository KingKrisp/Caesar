# Caesar Kobo installation

## Current validation

Caesar was installed on a Kobo Clara HD (model 376), reporting firmware
4.8.10959. The owner confirmed boot, book reading, Gallery, Notes and Settings.
The compatible application names remain `plato` / `plato.sh`.

The latest installation adds file rename/delete actions, clearer Settings,
a `Books`-only library view and persistent Caesar startup with recovery. Numeric
series sorting handles decimal indices such as `10.0` and `11.0`. A copy of the
device metadata was checked: 59 book entries, with all 13 *A Series of
Unfortunate Events* books ordered numerically from 1 through 13.

Validation executed locally:

- ARM release build and legacy-loader startup check under QEMU.
- 54 ARM core tests plus a UI flow test covering portrait and landscape.
- Seven recovery-launcher tests, including persistent startup, SIGSEGV fallback,
  and recovery after an incomplete startup.
- Offline script checks: three passed, one Linux-interface check skipped on macOS.

These checks do not establish permanent radio shutdown by unrelated boot
services. Full lighting, sleep/wake and every preference still need hardware
checks when changing those paths. The system startup script was not inspected
or modified; the compatible application entry point was verified on hardware.

## Build the ARM version

Follow [BUILD.md](BUILD.md). Apple Silicon instructions use the official old
Kobo sysroot with native Clang/LLD and preserve the original runtime libraries.
A desktop `plato` build is not a Kobo build.

The first Mac-produced ARM build failed before `main` because LLD separated two
dynamic relocation tables that the old ARM loader expects to be adjacent. The
tracked linker layout and Mac build helper prevent that failure. Do not remove
the layout correction or deploy an unchecked replacement binary.

## Preserve the existing library

Keep the library root at `/mnt/onboard`. Set `content-path = "Books"` in its
`[[libraries]]` entry; [Library-sample.toml](../contrib/Library-sample.toml) shows
that entry. This filters browsing/import without relocating books, notes,
captures, metadata or reading states. See [LIBRARY.md](LIBRARY.md).

To retain that folder choice across an emergency stock-Plato boot, put `Books`
on one line in `.adds/plato/.caesar-content-path`. The recovery launcher passes
it to Caesar if the newer Settings.toml field has been lost.

## Install with recovery

1. Preserve a verified backup of the working application, Settings.toml, library
   metadata and reading states. Before system-partition changes, image the whole
   internal card while unmounted; a USB file copy is not a full-card image.
2. Retain the original working Plato ELF as `.adds/plato/plato-stock`. Place the
   checked ARM Caesar ELF at `.adds/plato/plato-caesar`, alongside the matching
   resources and libraries. Keep the existing Settings.toml and custom fonts.
3. Install [recovery-launcher.sh](../contrib/recovery-launcher.sh) as
   `.adds/plato/plato`, retaining the compatible outer `plato.sh` launch path.
4. With neither `.caesar-enabled` nor `.caesar-trial` present, verify that the
   wrapper boots stock Plato and records this in `.caesar-recovery.log`.
5. Create `.caesar-trial` for a one-time hardware test. After it passes, create
   `.caesar-enabled` to keep Caesar selected across normal restarts.
6. Safely eject, restart, and check the application, book order and USB access.
   The log should show Caesar startup and `info.log` should include
   `Caesar startup complete.`. See [RECOVERY.md](RECOVERY.md).

The launcher falls back to stock after a failed process or a startup left
incomplete across a forced restart. USB auto-sharing still requires a running
reader app; it cannot rescue a failure before startup on its own. If USB is
unavailable, Clara HD's internal microSD can be accessed through a card reader.

An ordinary app update must not format, repartition, flash an unrelated full-card
image, or overwrite device-specific boot data. Do not replace the system startup
script or install a generic KoboRoot.tgz blindly. Keep personal books, notes,
settings, logs, databases and backup images out of the source repository.
