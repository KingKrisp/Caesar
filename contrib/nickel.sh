#! /bin/sh
# Overwrite old launcher copies during upgrades. Caesar stays offline.
"$(dirname "$0")/scripts/wifi-disable.sh"
echo 'Starting the stock Kobo interface is disabled in Caesar.' >&2
exit 1
