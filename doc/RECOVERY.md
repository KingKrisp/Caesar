# Caesar recovery launcher

The optional `contrib/recovery-launcher.sh` is installed as `.adds/plato/plato`.
Keep the stock executable as `plato-stock` and Caesar as `plato-caesar`, with the
usual resources and libraries in the same application directory.

- `.caesar-enabled` selects Caesar on every normal startup.
- `.caesar-trial` selects Caesar once and is consumed before launch.
- Without either file, the launcher starts stock Plato.

The launcher creates `.caesar-starting` before starting Caesar. After building
and rendering its first screen, Caesar removes that marker. If startup hangs
and the reader is forcibly restarted, the remaining marker disables Caesar and
selects stock Plato. An error exit or signal also disables Caesar and starts
stock Plato immediately. A normal successful exit retains the persistent choice
and lets the outer `plato.sh` handle shutdown or reboot.

The log is `.caesar-recovery.log`. To select stock manually while USB storage is
accessible, remove `.caesar-enabled` and `.caesar-trial`, then restart. Retain a
backup and verify the wrapper is on the actual startup path before relying on it.
This recovery behavior does not make USB sharing available before an app starts.
