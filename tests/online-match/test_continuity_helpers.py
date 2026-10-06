"""Helper contracts only; no browser/HTTPS/PostgreSQL acceptance is implied."""
import json
from types import SimpleNamespace
import unittest
from playwright.sync_api import Error as BrowserError
from browser_acceptance import admission_response_facts, exception_class, protected_endpoint_class
from unittest import mock
from continuity_acceptance import (AcceptanceFailure, CURRENT_BOARD, RECOVERING_CONCEALED,
    PageNetwork, Pair, command_identity, pending_record, run, current_context_after_restart, FOCUS_OBSERVER)


class ContinuityHelperTests(unittest.TestCase):
    def test_create_join_diagnostics_are_fixed_classes_without_private_routing(self):
        self.assertEqual(protected_endpoint_class('https://localhost:9443/api/v1/matches'),'create')
        self.assertEqual(protected_endpoint_class('https://localhost:9443/api/v1/matches/join'),'join')
        self.assertIsNone(protected_endpoint_class('https://localhost:9443/api/v1/matches?secret=synthetic'))
        self.assertIsNone(protected_endpoint_class('https://foreign.invalid/api/v1/matches'))

    def test_browser_protocol_errors_never_return_raw_secret_text(self):
        for fragment,expected in (('No resource with given identifier found','response_body_unavailable'),
                                  ('Request content was evicted from inspector cache','response_body_evicted'),
                                  ('Element is not attached to the DOM','browser_element_detached'),
                                  ('Response.json: synthetic failure','response_json_failed')):
            result=exception_class(BrowserError(fragment+' cookie=synthetic-secret'))
            self.assertEqual(result,expected);self.assertNotIn('secret',result)
        self.assertEqual(exception_class(BrowserError('arbitrary synthetic-secret')),'browser_error')

    def test_admission_metadata_discards_headers_and_bounds_announced_length(self):
        response=mock.Mock(status=200)
        values={'content-type':'application/json; charset=utf-8','cache-control':'private, no-store','content-length':'100'}
        response.header_value.side_effect=values.get
        self.assertEqual(admission_response_facts(response),{'status':200,'content_type_class':'json','no_store':True,'body_length_class':'bounded','bounded_body_length':100})
        for value,expected in ((None,'missing'),('synthetic-secret','invalid'),('2097153','over_budget')):
            values['content-length']=value;values['content-type']='private/synthetic-secret'
            facts=admission_response_facts(response)
            self.assertEqual(facts['body_length_class'],expected);self.assertNotIn('secret',json.dumps(facts))

    def test_first_focus_witness_reads_real_pixels_without_manufacturing_clear(self):
        for predicate in ('event.isTrusted','g.readPixels','g.FRAMEBUFFER_BINDING','cleared_pixels',
                          'selection_clear','aria-hidden',"visibility==='hidden'",'rangeCount','textContent.trim()',
                          '!facts.awaiting', 'facts.immediate!==null', '!event.isTrusted'):
            self.assertIn(predicate,FOCUS_OBSERVER)
        for mutation in ('g.clear(', 'g.bindFramebuffer(', 'c.width=', 'dispatchEvent('):
            self.assertNotIn(mutation,FOCUS_OBSERVER)

    def test_disposable_opt_in_precedes_supervisor_or_browser_setup(self):
        with mock.patch.dict('os.environ',{},clear=True),mock.patch('continuity_acceptance.SupervisorClient') as supervisor:
            with self.assertRaises(AcceptanceFailure):run(SimpleNamespace())
            supervisor.assert_not_called()

    def test_restart_context_read_requires_disposable_ci_before_credentials(self):
        with mock.patch.dict('os.environ',{},clear=True),mock.patch('continuity_acceptance.http.client.HTTPSConnection') as connection:
            with self.assertRaises(AcceptanceFailure):current_context_after_restart(None,'synthetic-cookie','synthetic-account')
            connection.assert_not_called()

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
