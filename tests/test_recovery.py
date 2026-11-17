"""Persistent Caesar startup and recovery behavior, without touching a Kobo."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

LAUNCHER = Path(__file__).resolve().parents[1] / "contrib/recovery-launcher.sh"

class Recovery(unittest.TestCase):
    def run_case(self, enabled=False, trial=False, incomplete=False, content_path=None, body='rm -f "$CAESAR_STARTUP_MARKER"\nexit 0', runs=1):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);shutil.copy2(LAUNCHER,root/'plato')
            if content_path is not None:(root/'.caesar-content-path').write_text(content_path+'\n')
            for name,content in [('plato-stock','echo stock'),('plato-caesar','echo caesar\n'+body)]:
                p=root/name;p.write_text('#!/bin/sh\n'+content+'\n');p.chmod(0o755)
            for present,name in [(enabled,'.caesar-enabled'),(trial,'.caesar-trial'),(incomplete,'.caesar-starting')]:
                if present:(root/name).touch()
            results=[subprocess.run(['sh',str(root/'plato')],capture_output=True,text=True) for _ in range(runs)]
            return results,{p.name for p in root.iterdir()},(root/'.caesar-recovery.log').read_text()
    def test_persistent_caesar_stays_selected_across_restarts(self):
        results,files,_=self.run_case(enabled=True,runs=2)
        self.assertEqual([r.stdout for r in results],['caesar\n','caesar\n'])
        self.assertIn('.caesar-enabled',files);self.assertNotIn('.caesar-starting',files)
    def test_stock_is_default_without_enable_or_trial(self):
        self.assertEqual(self.run_case()[0][0].stdout,'stock\n')
    def test_one_time_trial_remains_one_time(self):
        results,files,_=self.run_case(trial=True,runs=2)
        self.assertEqual([r.stdout for r in results],['caesar\n','stock\n'])
        self.assertNotIn('.caesar-trial',files)
    def test_crash_disables_caesar_and_recovers_now_and_next_boot(self):
        results,files,log=self.run_case(enabled=True,body='kill -SEGV $$',runs=2)
        self.assertEqual([r.stdout for r in results],['caesar\nstock\n','stock\n'])
        self.assertNotIn('.caesar-enabled',files);self.assertIn('exit 139',log)
    def test_incomplete_startup_recovers_on_forced_restart(self):
        results,files,log=self.run_case(enabled=True,incomplete=True)
        self.assertEqual(results[0].stdout,'stock\n')
        self.assertNotIn('.caesar-enabled',files);self.assertIn('did not complete',log)
    def test_folder_choice_is_exported_to_caesar(self):
        results,_,_=self.run_case(enabled=True,content_path='Books',body='echo "$CAESAR_CONTENT_PATH"\nrm -f "$CAESAR_STARTUP_MARKER"\nexit 0')
        self.assertEqual(results[0].stdout,'caesar\nBooks\n')
    def test_application_error_recovers(self):
        self.assertEqual(self.run_case(enabled=True,body='exit 1')[0][0].stdout,'caesar\nstock\n')

if __name__=='__main__':unittest.main()
