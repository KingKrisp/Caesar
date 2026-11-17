# Caesar local installation handoff

This checkout contains Caesar's source changes. It is **not a device-tested Kobo
release** and does not include the cloud workspace's compiled desktop binaries or
its locally installed build dependencies.

## Implemented and checked

- Numeric series sorting, including decimal indices such as `11.0`.
- Applications → Gallery for screenshots and saved sketches.
- Applications → Notes with a multiline keyboard editor, Markdown files, and preview.
- Settings with persistent preferences, reader bindings, and global shortcuts.
- Offline-only operation, disabled fetcher hooks, and deny stubs for old Wi-Fi
  enable and stock-interface launcher scripts.

Validation completed in the cloud workspace:

```sh
cargo test --locked -p plato-core
python3 tests/test_offline.py
cargo build --locked -p emulator -p plato -p importer
```

The 49 core tests and 4 mocked radio/launcher tests passed. The desktop builds
passed. Emulator checks used Clara HD resolution and covered saved settings,
disabled/remapped reader inputs, orientation and dark mode across restarts,
gallery navigation, and note typing, formatting, saving, reopening, and sleep saving.
The radio tests mock system commands and do not establish hardware radio state.

See [BUILD.md](BUILD.md) for dependencies and the separate Kobo ARM build process.
The Kobo target is `arm-unknown-linux-gnueabihf`; the successful host build of the
`plato` package is not a substitute for that ARM build. Binary and launcher names
remain `plato` / `plato.sh` for compatibility; the application identifies as Caesar.

## Existing device setup

The owner's Kobo already boots directly into Plato through a modified system
startup script, possibly `rcS`. The exact model, firmware, script path, and script
contents still need inspection. The likely model is Clara HD, not yet confirmed.
No device boot files, partitions, or SD-card contents were modified in the cloud.
Preserve direct boot into the reader when installing Caesar.

## Local installation work still required

1. Identify the SD card and its partitions without changing them. Confirm the
   device model and firmware from its files.
2. Before writing, make and verify a full image of the entire card while its
   partitions are unmounted. Store the image on another drive. A file copy alone
   does not preserve partition tables and boot data.
3. Inspect the actual boot script and installation paths. Keep backups of the
   working Plato installation, configuration, reading states, books, notes,
   sketches, and boot script, with a concrete rollback procedure.
4. Build and check the ARM binary, its shared libraries, and distribution contents
   for the confirmed Kobo. Do not copy desktop binaries onto the card.
5. Review launcher behavior before installation: Caesar's packaged launcher powers
   off on exit instead of launching Nickel. Its `nickel.sh` and `wifi-enable.sh`
   are deny stubs. Integrate these semantics with the existing direct-boot script;
   do not blindly replace the device's system startup file.
6. Install with the rollback files preserved, then test boot, reading, touch,
   lighting, sleep/wake, USB mass storage, and the new apps on the actual Kobo.
7. Verify radio/interface shutdown and inspect any other boot services before
   declaring the whole device permanently offline. Caesar's software policy does
   not by itself verify what unrelated firmware or startup services do.

The local Codex session needs filesystem access to the mounted card. Do not format,
repartition, flash a full image, or overwrite device-specific boot data as part of
an ordinary application installation.
