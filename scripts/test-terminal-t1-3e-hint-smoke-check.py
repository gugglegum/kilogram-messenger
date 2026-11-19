"""Focused scope/order tests for the adapter; no nodes, transports or product reruns."""
import runpy,unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
M=runpy.run_path(str(Path(__file__).with_name('test-terminal-t1-3e-hint-smoke.py')))
class Checks(unittest.TestCase):
    def fixture(self):
        smoke=object.__new__(M['HintSmoke']);smoke.end=100;smoke.messages=[];smoke.processes=[];calls=[]
        smoke.monitors=lambda *a,**k:calls.append(('monitors',k))
        def send(sender,slot,label,end):
            row=dict(sender=sender,slot=slot);smoke.messages.append(row);calls.append(('send',sender,slot));return row
        smoke.send=send;smoke.deliver=lambda *a:calls.append(('delivered',));smoke.histories=lambda *a:calls.append(('histories',))
        smoke.processes=[(0,SimpleNamespace(events=[1,2])),(1,SimpleNamespace(events=[3,4]))]
        return smoke,calls
    def test_admit_then_exact_four_both_directions_and_slots(self):
        smoke,calls=self.fixture()
        with patch.object(M['M']['Diagnostic'],'functional',lambda self:calls.append(('admitted',))),patch.dict(M['D']['prior'],incoming_result=lambda *a:dict(status='confirmed')):
            smoke.functional()
        self.assertEqual(calls[0],('admitted',));self.assertEqual([r for r in calls if r[0]=='send'],[('send',0,0),('send',1,0),('send',0,1),('send',1,1)])
        self.assertEqual(sum(r[0]=='delivered' for r in calls),4);self.assertEqual(calls[-1],('histories',))
    def test_failed_admission_sends_nothing(self):
        smoke,calls=self.fixture()
        with patch.object(M['M']['Diagnostic'],'functional',side_effect=RuntimeError('admission')):
            with self.assertRaisesRegex(RuntimeError,'admission'):smoke.functional()
        self.assertEqual(calls,[]);self.assertEqual(smoke.messages,[])
if __name__=='__main__':unittest.main()
