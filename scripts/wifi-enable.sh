#! /bin/sh
# Compatibility stub overwrites the old script during an in-place installation.
# Nothing may bring the radio up in Caesar.
"$(dirname "$0")/wifi-disable.sh"
echo 'Caesar is offline. Wi-Fi cannot be enabled.' >&2
exit 1
