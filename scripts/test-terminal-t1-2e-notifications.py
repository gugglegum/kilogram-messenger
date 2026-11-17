"""Focused checks for offline notification evidence and the phase B verdict."""
from pathlib import Path
import runpy
from types import SimpleNamespace
import unittest
from unittest.mock import patch

driver = runpy.run_path(str(Path(__file__).with_name("test-terminal-t1-2e-driver.py")))
incoming_result = driver["incoming_result"]
observe_incoming = driver["observe_incoming"]
notifications_confirmed = driver["notifications_confirmed"]


class Clock:
    now = 0

    def monotonic(self):
        return self.now

    def sleep(self, duration):
        self.now += duration


class NotificationProof(unittest.TestCase):
    def setUp(self):
        self.clock = Clock()
        self.notice = {"event_id": "offline-event", "context": "dialog-2", "text": "message",
                       "runtime_instance": "restarted-receiver", "epoch": "watching-epoch"}
        self.row = {"event_id": self.notice["event_id"], "key": {"context": self.notice["context"]},
                    "text": self.notice["text"], "receiver_instance": self.notice["runtime_instance"],
                    "receiver_epoch": self.notice["epoch"], "delivered_before_fresh_exchange": True}
        replacement = patch.dict(observe_incoming.__globals__, time=self.clock)
        replacement.start()
        self.addCleanup(replacement.stop)

    def test_early_without_incoming_does_not_pass(self):
        result = observe_incoming(SimpleNamespace(events=[]), self.row, deadline=90)
        self.assertEqual(result, {"status": "not_observed", "events": []})
        self.assertEqual(self.clock.now, 35)
        self.assertFalse(notifications_confirmed([{**self.row, "notification": result}]))

    def test_early_with_one_incoming_passes(self):
        result = observe_incoming(SimpleNamespace(events=[self.notice]), self.row, deadline=90)
        self.assertEqual(result, {"status": "confirmed", "events": [self.notice]})
        self.assertTrue(notifications_confirmed([{**self.row, "notification": result}]))

    def test_delayed_incoming_is_seen_even_when_delivery_was_early(self):
        clock, notice = self.clock, self.notice

        class Receiver:
            @property
            def events(self):
                return [notice] if clock.now >= 1 else []

        result = observe_incoming(Receiver(), self.row, deadline=90)
        self.assertEqual(result["status"], "confirmed")
        self.assertEqual(self.clock.now, 1)

    def test_whole_phase_deadline_bounds_wait(self):
        result = observe_incoming(SimpleNamespace(events=[]), self.row, deadline=2)
        self.assertEqual(result["status"], "not_observed")
        self.assertEqual(self.clock.now, 2)

    def test_late_duplicate_revokes_confirmation_after_streams_drain(self):
        initial = observe_incoming(SimpleNamespace(events=[self.notice]), self.row, deadline=90)
        self.assertEqual(initial["status"], "confirmed")
        drained = incoming_result([self.notice, self.notice], self.row)
        self.assertEqual(drained["status"], "duplicate")
        self.assertFalse(notifications_confirmed([{**self.row, "notification": drained}]))

    def test_matching_event_with_wrong_epoch_is_not_confirmed(self):
        result = incoming_result([{**self.notice, "epoch": "older-epoch"}], self.row)
        self.assertEqual(result["status"], "binding_mismatch")
        self.assertFalse(notifications_confirmed([{**self.row, "notification": result}]))


if __name__ == "__main__":
    unittest.main(verbosity=2)
