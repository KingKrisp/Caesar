#!/bin/sh
# Installed as .adds/plato/plato; retain the original ELF as plato-stock.
WORKDIR=$(CDPATH= cd -- "$(dirname "$0")" && pwd) || exit 1
cd "$WORKDIR" || exit 1
log() { printf 'Recovery launcher v2: %s\n' "$*" >> .caesar-recovery.log; }
export LD_LIBRARY_PATH="$WORKDIR/libs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
if [ -f .caesar-content-path ]; then
    CAESAR_CONTENT_PATH=$(cat .caesar-content-path)
    export CAESAR_CONTENT_PATH
fi
log invoked.
if [ -e .caesar-starting ]; then
    log 'previous startup did not complete; disabling Caesar.'
    rm -f .caesar-enabled .caesar-trial .caesar-starting
    sync
fi
if [ -f .caesar-enabled ] || [ -f .caesar-trial ]; then
    rm -f .caesar-trial || exit 1
    : > .caesar-starting || exit 1
    sync
    export CAESAR_STARTUP_MARKER="$WORKDIR/.caesar-starting"
    log 'starting Caesar.'
    ./plato-caesar
    status=$?
    if [ "$status" -eq 0 ]; then
        rm -f .caesar-starting
        sync
        exit 0
    fi
    log "Caesar failed (exit $status); disabling Caesar."
    rm -f .caesar-enabled .caesar-starting
    sync
fi
log 'starting stock Plato.'
exec ./plato-stock
