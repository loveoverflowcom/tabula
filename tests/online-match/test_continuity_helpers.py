"""Helper contracts only; no browser/HTTPS/PostgreSQL acceptance is implied."""
import json
from types import SimpleNamespace
import unittest
from unittest import mock
from continuity_acceptance import (AcceptanceFailure, CURRENT_BOARD, RECOVERING_CONCEALED,
    PageNetwork, Pair, command_identity, pending_record, run)


class ContinuityHelperTests(unittest.TestCase):
    def test_disposable_opt_in_precedes_supervisor_or_browser_setup(self):
        with mock.patch.dict('os.environ',{},clear=True),mock.patch('continuity_acceptance.SupervisorClient') as supervisor:
            with self.assertRaises(AcceptanceFailure):run(SimpleNamespace())
            supervisor.assert_not_called()

    def test_exact_command_identity_preserves_u128_and_original_sequence(self):
        body='{"version":2,"attachment_id":"'+'a'*32+'","command":{"seq":1,"command":{"match_id":340282366920938463463374607431768211455}}}'
        got=command_identity(body)
        self.assertEqual(got['command']['match_id'],2**128-1)
        self.assertEqual(got['seq'],1)
        with self.assertRaises(AcceptanceFailure):command_identity(body.replace('"version":2','"version":1'))

    def test_pending_hints_exclude_private_projection_and_auth_fields(self):
        valid={'version':2,'match_id':'a'*32,'game_id':'x','game_version':'1','operation_scope':'b'*64,'command':'opaque','expires_at':10}
        page=mock.Mock();page.evaluate.return_value=json.dumps(valid)
        self.assertEqual(pending_record(page,'a'*32),valid)
        for field in ('csrf_token','binding_id','view','seed','ledger','state_hash'):
            page.evaluate.return_value=json.dumps(dict(valid,**{field:'never retained'}))
            with self.assertRaises(AcceptanceFailure):pending_record(page,'a'*32)
        page.evaluate.return_value=None;self.assertIsNone(pending_record(page,'a'*32))

    def test_page_scoped_network_injection_uses_real_cdp_and_restores_connectivity(self):
        page=mock.Mock();session=page.context.new_cdp_session.return_value
        network=PageNetwork(page);network.offline(True);network.close()
        page.context.new_cdp_session.assert_called_once_with(page)
        session.send.assert_has_calls([mock.call('Network.enable'),mock.call('Network.emulateNetworkConditions',{'offline':True,'latency':0,'downloadThroughput':-1,'uploadThroughput':-1}),mock.call('Network.emulateNetworkConditions',{'offline':False,'latency':0,'downloadThroughput':-1,'uploadThroughput':-1})])
        session.detach.assert_called_once()

    def test_recovery_concealment_requires_actual_privacy_conditions(self):
        for predicate in ("onlineAvailability==='recovering'","!Object.hasOwn", "visibility==='hidden'", "aria-hidden", "textContent.trim()===''"):
            self.assertIn(predicate,RECOVERING_CONCEALED)
        self.assertNotIn('canvas.hidden',RECOVERING_CONCEALED)
        self.assertIn("d.onlineConnection?.startsWith('Read-only')",CURRENT_BOARD)
        self.assertIn("d.onlineSeat===String(seat)",CURRENT_BOARD)

    def test_attachment_observer_rejects_wrong_seat_and_canonical_payload(self):
        pair=Pair.__new__(Pair);pair.match_id='a'*32;pair.attachments=[[],[]]
        response=mock.Mock(status=200,url='https://localhost:9443/api/v1/matches/'+pair.match_id+'/attach')
        response.json.return_value={'version':2,'seat':1}
        with self.assertRaises(AcceptanceFailure):pair.observe_attach(response,0)
        response.json.return_value={'version':2,'seat':0,'seed':[1]}
        with self.assertRaises(AcceptanceFailure):pair.observe_attach(response,0)
        self.assertEqual(pair.attachments,[[],[]])
        response.json.return_value={'version':2,'seat':0,'frames':[]}
        pair.observe_attach(response,0);self.assertEqual(len(pair.attachments[0]),1)

if __name__=='__main__':unittest.main()
