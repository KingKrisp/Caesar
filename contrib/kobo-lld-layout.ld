/* Keep dynamic relocation tables adjacent for the legacy ARM loader. */
SECTIONS {
  .ARM.exidx : { *(.ARM.exidx*) }
}
INSERT AFTER .rel.plt;
