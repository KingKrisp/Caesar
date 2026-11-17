## Modes

With both modes, the reading states are stored within the `.reading-states` directory.

### Database

The files and directories are read from a cached portion of the filesystem — the database — built and updated during the import phase, stored in `.metadata.json`.

The shelf displays the descendants of the current directory.

### Filesystem

The files and directories are read directly from the filesystem.

The shelf displays the direct children of the current directory.

## Import Metadata

You can use `plato-import` to off-load the import task to a computer.

You can import with `plato-import -I LIBRARY_PATH`.

If new entries were added, you might populate the metadata with `plato-import -a ADDED_DATETIME -E LIBRARY_PATH` where the argument passed to `-a` is the added date-time of the first added entry (the new entries are at the bottom of the database).

You can then edit the database with your text editor to manually fix the metadata.

## Library Backups

You can make a backup of a library with:

```sh
rsync -vurt --delete EREADER_LIBRARY_PATH/ COMPUTER_LIBRARY_PATH/
```

## Keep the book list in one folder

A library can keep its existing storage and reading history while limiting the
book browser and automatic import to a subfolder:

```toml
[[libraries]]
name = "On Board"
path = "/mnt/onboard"
content-path = "Books"
mode = "database"
```

`content-path` is relative to `path`. Both database browsing and filesystem
browsing/search respect it. Previously indexed entries outside that folder stay
in the database but are hidden from the book list. Files and reading states are
not moved or deleted. Gallery and Notes continue to use the library's original
root. Without `content-path`, the library behaves as before.

For installations that can recover into an older Plato build, put `Books` on
one line in `.caesar-content-path` in the application directory. The recovery
launcher exports this as `CAESAR_CONTENT_PATH`. Caesar uses it for the selected
library when `content-path` is absent. This retains the folder choice if the
older reader rewrites Settings.toml without recognizing the new field.
