#! /bin/sh
# No pre/post hooks: an old user hook must not re-enable the radio.
command -v rfkill >/dev/null 2>&1 && rfkill block wlan
killall -q wpa_supplicant udhcpc default.script dhcpcd dhcpcd-dbus

# Shut interfaces down even if the driver/module environment is missing.
status=0
for interface in "${INTERFACE:-wlan0}" /sys/class/net/*; do
    interface=${interface##*/}
    [ "$interface" != lo ] || continue
    [ -e "/sys/class/net/$interface" ] || continue
    command -v wpa_cli >/dev/null 2>&1 && wpa_cli -i "$interface" terminate
    ifconfig "$interface" down
    flags=$(cat "/sys/class/net/$interface/flags" 2>/dev/null)
    [ -z "$flags" ] || [ "$((flags & 1))" -eq 0 ] || status=1
done

case "${WIFI_MODULE:-}" in
    dhd) rmmod dhd ;;
    moal) rmmod moal; rmmod mlan; ioctl -q -v 0 /dev/ntx_io 208 ;;
    wlan_drv_gen4m) rmmod wlan_drv_gen4m; [ ! -e /dev/wmtWifi ] || echo 0 > /dev/wmtWifi ;;
    "") : ;;
    *) rmmod "$WIFI_MODULE" ;;
esac
rmmod sdio_wifi_pwr 2>/dev/null || :
exit "$status"
