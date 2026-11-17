#!/usr/bin/env python3
"""Exercise the radio scripts with fake commands; never change host networking."""
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
import unittest


class OfflineScripts(unittest.TestCase):
    def run_script(self, name, flags="0x0"):
        with tempfile.TemporaryDirectory(prefix="caesar-offline-") as directory:
            root = Path(directory)
            commands = root / "commands"
            stub = '#!/bin/sh\nname=${0##*/}\nprintf "%s %s\\n" "$name" "$*" >> "$CAESAR_TEST_LOG"\n[ "$name" != cat ] || printf "%s\\n" "$CAESAR_TEST_FLAGS"\nexit 0\n'
            for name_of_command in ("rfkill", "killall", "wpa_cli", "ifconfig", "rmmod", "ioctl", "cat"):
                path = root / name_of_command
                path.write_text(stub)
                path.chmod(0o755)
            env = dict(os.environ, PATH=f"{root}:/usr/bin:/bin", WIFI_MODULE="dhd", INTERFACE="eth0",
                       CAESAR_TEST_LOG=str(commands), CAESAR_TEST_FLAGS=flags)
            repo = Path(__file__).resolve().parents[1]
            application = root / "application"
            (application / "scripts").mkdir(parents=True)
            for script in ("wifi-enable.sh", "wifi-disable.sh"):
                shutil.copy2(repo / "scripts" / script, application / "scripts" / script)
            shutil.copy2(repo / "contrib" / "nickel.sh", application / "nickel.sh")
            target = application / name if name == "nickel.sh" else application / "scripts" / name
            result = subprocess.run(["/bin/sh", str(target)], env=env,
                                    capture_output=True, text=True, check=False)
            return result, commands.read_text() if commands.exists() else ""

    def test_shutdown_blocks_radio_without_up_hooks(self):
        result, commands = self.run_script("wifi-disable.sh")
        self.assertEqual(result.returncode, 0)
        self.assertIn("rfkill block wlan", commands)
        self.assertIn("killall -q wpa_supplicant", commands)
        self.assertNotIn(" up", commands)
        self.assertNotIn("ifconfig lo down", commands)
        self.assertIn("rmmod dhd", commands)

    def test_old_enable_script_refuses_to_enable_networking(self):
        result, commands = self.run_script("wifi-enable.sh")
        self.assertEqual(result.returncode, 1)
        self.assertIn("cannot be enabled", result.stderr)
        self.assertIn("rfkill block wlan", commands)
        self.assertNotIn(" up", commands)

    def test_legacy_stock_interface_entry_point_is_disabled(self):
        result, commands = self.run_script("nickel.sh")
        self.assertEqual(result.returncode, 1)
        self.assertIn("stock Kobo interface is disabled", result.stderr)
        self.assertIn("rfkill block wlan", commands)
        self.assertNotIn(" up", commands)

    @unittest.skipUnless(Path("/sys/class/net/eth0").exists(), "needs an interface name for mocked flag checking")
    def test_still_up_interface_is_reported_as_failure(self):
        result, _ = self.run_script("wifi-disable.sh", flags="0x1")
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
