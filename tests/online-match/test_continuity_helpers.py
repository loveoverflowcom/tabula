"""Helper contracts only; no browser/HTTPS/PostgreSQL acceptance is implied."""
import json
from types import SimpleNamespace
import unittest
from playwright.sync_api import Error as BrowserError, TimeoutError as BrowserTimeout
from actual_response import ObservationFailure
from browser_acceptance import admission_response_facts, exception_class, protected_endpoint_class
from unittest import mock
from continuity_acceptance import (AcceptanceFailure, CURRENT_BOARD, RECOVERING_CONCEALED,
    PageNetwork, Pair, command_identity, pending_record, run, pair_diagnostics, current_context_after_restart, FOCUS_OBSERVER,
    same_record_rotation_game, focus_only_revoke_game, held_prefix,
    crash_game, apply_and_committed_refresh_game, restart_between_grant_and_attach_game,
    restart_with_fresh_attachment, network_refresh_game)


class ContinuityHelperTests(unittest.TestCase):
    def test_rotation_prepares_control_before_outage_and_restores_after_current_authority_witnesses(self):
        # Coordination only: no mock response supplies actual browser evidence.
        events=[];white,black,control=mock.Mock(),mock.Mock(),mock.Mock()
        context=mock.Mock();context.new_page.return_value=control
        context.cookies.side_effect=[[{'name':'__Host-tabula_session','value':'old'}],
                                     [{'name':'__Host-tabula_session','value':'new'}]]
        control.goto.side_effect=lambda *args,**kwargs:events.append(('control_ready',))
        scope='b'*64;match_id='a'*32;command={'seq':1}
        pair=mock.Mock(pages=[white,black],match_id=match_id,
            facts=[{'account_id':'white'},{'account_id':'black'}],
            attachments=[[{'operation_scope':scope,'attachment_id':'old'}],[{}]])
        pair.tap.side_effect=lambda *args:(events.append(('pointer_command',)) or
            json.dumps({'version':2,'command':command}))
        pair.current_status.side_effect=lambda role,status:events.append(('status',role,status))
        pair.board.side_effect=lambda role,status:events.append(('pixels',role,status))
        pair.oracle.side_effect=lambda expected,role=1:events.append(('oracle',expected,role))
        callbacks={}
        white.on.side_effect=lambda name,callback:callbacks.update({name:callback})
        network=mock.Mock()
        def offline(value):
            events.append(('offline',value))
            if not value:
                response=mock.Mock(url=f'https://localhost:9443/api/v1/matches/{match_id}/command',status=200)
                response.request.post_data=json.dumps({'version':2,'command':command,'attachment_id':'fresh'})
                callbacks['response'](response)
        network.offline.side_effect=offline
        def api_call(page,path,*args):
            if path=='/api/v1/auth/refresh':
                events.append(('credential_rotation',));return {'status':204,'body':None}
            events.append(('stale_probe',));return {'status':409,'body':{'code':'reattach_required'}}
        with mock.patch('continuity_acceptance.Pair',return_value=pair), \
             mock.patch('continuity_acceptance.PageNetwork',return_value=network), \
             mock.patch('continuity_acceptance.held'), \
             mock.patch('continuity_acceptance.pending_record',return_value={'operation_scope':scope}), \
             mock.patch('continuity_acceptance.fault_control',return_value={'status':200}), \
             mock.patch('continuity_acceptance.context_facts',return_value={'account_id':'white','csrf_token':'A'*43}), \
             mock.patch('continuity_acceptance.api',side_effect=api_call), \
             mock.patch('continuity_acceptance.move'), \
             mock.patch('continuity_acceptance.reattach_required',side_effect=lambda *args:events.append(('stale_rejected',))), \
             mock.patch('continuity_acceptance.restored_same_scope'), \
             mock.patch('continuity_acceptance.actual_response_json',return_value={'frames':[{'body':{'Ack':{'seq':1}}}]}):
            cases=[];same_record_rotation_game([context,mock.Mock()],mock.Mock(),mock.Mock(),cases)
        self.assertLess(events.index(('control_ready',)),events.index(('pointer_command',)))
        restored=events.index(('offline',False))
        for witness in (('credential_rotation',),('oracle',2,1),('stale_rejected',)):
            self.assertLess(events.index(witness),restored)
        self.assertLess(restored,events.index(('pixels',1,'White to move')))
        self.assertEqual([event[1:] for event in events if event[0]=='pixels'],[
            (0,'White to move'),(1,'Black to move'),(1,'White to move'),(0,'White to move')])
        self.assertEqual(len(cases),1);pair.full.assert_called_once_with(2)
        white.remove_listener.assert_called_once_with('response',callbacks['response'])

    def test_network_restore_follows_durable_confirmation_before_other_browser_pixels(self):
        # Coordination doubles only: actual pixels, commits and retransmission
        # remain assertions in the real browser/PostgreSQL acceptance target.
        events=[];pages=[mock.MagicMock(),mock.MagicMock()]
        pair=mock.Mock(pages=pages,match_id='a'*32,attachments=[[
            {'frames':[{'body':{'MatchUpdate':{'revision':0,'view':'projected fixture'}}}]}],[{}]])
        pair.tap.return_value='unchanged-command';pair.arm.return_value='gate'
        pair.current_status.side_effect=lambda role,status:events.append(('status',role,status))
        pair.board.side_effect=lambda role,status:events.append(('pixels',role,status))
        pair.oracle.side_effect=lambda expected,role=1:events.append(('oracle',expected,role))
        def network(page):
            value=mock.Mock();role=pages.index(page)
            value.offline.side_effect=lambda offline:events.append(('offline',role,offline))
            return value
        with mock.patch('continuity_acceptance.Pair',return_value=pair), \
             mock.patch('continuity_acceptance.PageNetwork',side_effect=network), \
             mock.patch('continuity_acceptance.pending_record',side_effect=[{'version':2},None]), \
             mock.patch('continuity_acceptance.restored_same_scope'), \
             mock.patch('continuity_acceptance.held'), \
             mock.patch('continuity_acceptance.fault_control',return_value={'status':200}), \
             mock.patch('continuity_acceptance.move'):
            cases=[];network_refresh_game([],mock.Mock(),mock.Mock(),cases,mock.Mock())
        committed=events.index(('oracle',2,0))
        self.assertEqual(events[committed-1:committed+4],[('status',0,'White to move'),
            ('oracle',2,0),('offline',1,False),('pixels',0,'White to move'),('pixels',1,'White to move')])
        terminal=events.index(('oracle',4,1))
        self.assertEqual(events[terminal:terminal+5],[('oracle',4,1),('offline',0,False),
            ('pixels',1,'Game over / Black wins / checkmate'),('pixels',0,'Game over / Black wins / checkmate'),('oracle',4,1)])
        self.assertEqual(sum(event[0]=='pixels' for event in events),8)
        self.assertEqual(len(cases),3)

    def test_crash_restart_requires_a_new_request_and_completed_attachment_body(self):
        match_id='a'*32;scope='b'*64
        old={'version':2,'seat':1,'attachment_id':'c'*32,'operation_scope':scope,'frames':[]}
        fresh={**old,'attachment_id':'d'*32}
        page=mock.Mock();page.evaluate.return_value=100;listeners={};accepted=[]
        page.on.side_effect=lambda name,callback:listeners.setdefault(name,[]).append(callback)
        page.remove_listener.side_effect=lambda name,callback:listeners[name].remove(callback)
        def request(failure=None,start=101):
            value=mock.Mock(method='POST',url=f'https://localhost:9443/api/v1/matches/{match_id}/attach',failure=failure)
            value.timing={'startTime':start}
            value._impl_obj=object();value.response.return_value=mock.Mock(status=200)
            return value
        pre_crash=request(start=99);queued_old=request(start=99);header_only=request('net::ERR_ABORTED');current=request()
        class Selection:
            value=None
            def __init__(self,predicate):self.predicate=predicate
            def __enter__(self):return self
            def __exit__(self,*args):
                if not args[0] and self.value is None:raise BrowserTimeout('fresh attachment absent')
            def finish(self,request):
                accepted.append(self.predicate(request))
                if accepted[-1]:self.value=request
        selection=None
        def expect(event,*,predicate,timeout):
            nonlocal selection
            self.assertEqual((event,timeout),('requestfinished',60_000))
            selection=Selection(predicate);return selection
        page.expect_event.side_effect=expect
        def restart(mode):
            self.assertEqual(mode,'normal')
            selection.finish(pre_crash)
            for value in (queued_old,header_only,current):
                if value is current:value.timing={'startTime':-1}
                for callback in listeners['request']:callback(value)
                if value is current:value.timing={'startTime':101}
                selection.finish(value)
            return {'alive':True}
        supervisor=mock.Mock();supervisor.restart.side_effect=restart
        pair=SimpleNamespace(pages=[mock.Mock(),page],attachments=[[{}],[old]],match_id=match_id)
        with mock.patch('continuity_acceptance.actual_response_json',return_value=fresh) as observed:
            restart_with_fresh_attachment(pair,supervisor)
        self.assertEqual(accepted,[False,False,False,True])
        observed.assert_called_once_with(current.response.return_value)
        self.assertEqual(listeners['request'],[])

    def test_crash_restart_rejects_changed_operation_scope(self):
        previous={'attachment_id':'a'*32,'operation_scope':'b'*64}
        page=mock.MagicMock();page.evaluate.return_value=100;request=mock.Mock()
        page.expect_event.return_value.__enter__.return_value=SimpleNamespace(value=request)
        pair=SimpleNamespace(pages=[mock.Mock(),page],attachments=[[{}],[previous]],match_id='c'*32)
        supervisor=mock.Mock();supervisor.restart.return_value={'alive':True}
        with mock.patch('continuity_acceptance.completed_attachment_response',return_value=mock.Mock()), \
             mock.patch('continuity_acceptance.actual_response_json',return_value={
                 'version':2,'seat':1,'attachment_id':'d'*32,'operation_scope':'e'*64,'frames':[]}):
            with self.assertRaises(AcceptanceFailure):restart_with_fresh_attachment(pair,supervisor)
        page.remove_listener.assert_called_once()

    def test_held_prefix_uses_only_existing_ephemeral_token_and_known_public_expectation(self):
        reply={'status':200,'body':{'version':1,'expected_public_transcript_prefix':True}}
        with mock.patch('continuity_acceptance.wire_probe',return_value=reply) as request:
            for expected in range(5):
                held_prefix('synthetic-ca','synthetic-control-token',expected)
                ca,path,headers,body=request.call_args.args
                self.assertEqual(ca,'synthetic-ca')
                self.assertEqual(path,'/__fixture/continuity/held-prefix')
                self.assertEqual(headers,[('Origin','https://localhost:9443'),
                                          ('Content-Type','application/json'),
                                          ('X-Tabula-Fixture-Control','synthetic-control-token')])
                self.assertEqual(json.loads(body),{'version':1,'expected_inputs':expected})
            request.reset_mock()
            for invalid in (-1,5,True,1.0,'1',None):
                with self.subTest(expected=invalid),self.assertRaises(AcceptanceFailure):
                    held_prefix('synthetic-ca','synthetic-control-token',invalid)
            request.assert_not_called()

    def test_held_prefix_rejects_mismatch_errors_and_any_extra_or_non_boolean_output(self):
        valid={'version':1,'expected_public_transcript_prefix':True}
        bodies=[None,{},dict(valid,expected_public_transcript_prefix=False),
                dict(valid,expected_public_transcript_prefix=1),dict(valid,version=True),
                dict(valid,version=2)]
        bodies.extend(dict(valid,**{field:'synthetic-private'})
                      for field in ('state','view','seed','state_hash','input_index','count',
                                    'match_id','record','binding_id','control_token'))
        for status,body in [(200,value) for value in bodies]+[(503,valid),(403,valid),(409,valid),(410,valid)]:
            with self.subTest(status=status,body=body), \
                 mock.patch('continuity_acceptance.wire_probe',return_value={'status':status,'body':body}), \
                 self.assertRaises(AcceptanceFailure):
                held_prefix('synthetic-ca','synthetic-control-token',0)

    def test_held_witness_updates_native_audit_expectation_only_after_success(self):
        pair=Pair.__new__(Pair);pair.write_audit=mock.Mock()
        with mock.patch('continuity_acceptance.held_prefix',side_effect=AcceptanceFailure('fixed mismatch')):
            with self.assertRaises(AcceptanceFailure):pair.held_prefix(None,'synthetic-token',0)
            pair.write_audit.assert_not_called()
        with mock.patch('continuity_acceptance.held_prefix') as witness:
            pair.held_prefix(None,'synthetic-token',1)
            witness.assert_called_once_with(None,'synthetic-token',1)
            pair.write_audit.assert_called_once_with(1)

    def test_crash_apply_rotation_and_restart_witness_do_not_authenticate_while_held(self):
        class AfterHeldWitness(Exception):
            pass
        # Stop before injecting any transport/crash fault. These doubles check
        # harness coordination only, never actual browser or database acceptance.
        for operation,partition,expected in ((crash_game,False,0),(crash_game,True,1),
                (apply_and_committed_refresh_game,'before_commit',0),
                (apply_and_committed_refresh_game,'after_commit',1),
                (same_record_rotation_game,None,1),(restart_between_grant_and_attach_game,None,1)):
            pair=mock.Mock(pages=[mock.Mock(),mock.Mock()],attachments=[[{}],[{}]])
            pair.arm.return_value='synthetic-token'
            with self.subTest(operation=operation.__name__,partition=partition), \
                 mock.patch('continuity_acceptance.Pair',return_value=pair), \
                 mock.patch('continuity_acceptance.held') as confirmed, \
                 mock.patch('continuity_acceptance.PageNetwork',side_effect=AfterHeldWitness), \
                 mock.patch('continuity_acceptance.context_facts') as auth, \
                 mock.patch('continuity_acceptance.api') as api_call:
                with self.assertRaises(AfterHeldWitness):
                    contexts=[mock.Mock(),mock.Mock()]
                    if operation is crash_game:operation(contexts,None,'synthetic-ca',mock.Mock(),partition,[])
                    elif operation is apply_and_committed_refresh_game:operation(contexts,None,'synthetic-ca',partition,[])
                    elif operation is restart_between_grant_and_attach_game:operation(contexts,None,'synthetic-ca',mock.Mock(),[])
                    else:operation(contexts,None,'synthetic-ca',[])
                confirmed.assert_called_once_with('synthetic-ca','synthetic-token')
                pair.held_prefix.assert_called_once_with('synthetic-ca','synthetic-token',expected)
                pair.oracle.assert_not_called();auth.assert_not_called();api_call.assert_not_called()

    def test_board_matches_exact_game_owned_seat_wording(self):
        # Fixed projected facts are independent examples from Chess status_text.
        # This fake does not manufacture browser, TLS or rendered acceptance.
        class ProjectedPage:
            def __init__(self, seat, status, available=True, visible=True,
                         connection='Connected · server-authoritative'):
                self.seat=seat;self.status=status;self.available=available
                self.visible=visible;self.connection=connection;self.foreground=False
            def bring_to_front(self): self.foreground=True
            def wait_for_function(self, predicate, *, arg, timeout):
                self.last_arg=arg
                connection=(self.connection.startswith('Read-only') if arg['readonly']
                            else self.connection=='Connected · server-authoritative')
                if not (self.foreground and self.available and self.visible and connection
                        and self.seat==arg['seat'] and self.status==arg['status']):
                    raise BrowserTimeout('fixed projected board mismatch')

        pair=Pair.__new__(Pair);pair.trace=None
        examples=((0,'White to move','Your turn / White'),
                  (1,'White to move','White to move'),
                  (0,'Black to move','Black to move'),
                  (1,'Black to move','Your turn / Black'),
                  (0,'White to move / CHECK','Your turn / White / CHECK'),
                  (1,'Black to move / CHECK','Your turn / Black / CHECK'),
                  (0,'Black to move / CHECK','Black to move / CHECK'),
                  (1,'White to move / CHECK','White to move / CHECK'),
                  (0,'Game over / Black wins / checkmate','Game over / Black wins / checkmate'),
                  (1,'Game over / Black wins / checkmate','Game over / Black wins / checkmate'))
        for role,expected,actual in examples:
            with self.subTest(role=role,expected=expected), \
                 mock.patch('continuity_acceptance.rendered_canvas_pixels') as pixels:
                page=ProjectedPage(role,actual)
                pair.board(role,expected,page=page)
                self.assertEqual(page.last_arg,{'seat':role,'status':actual,'readonly':False})
                pixels.assert_called_once_with(page)

        # Neither observer wording on one's own turn nor another seat/authority
        # can satisfy the exact predicate. Missing CHECK stays a mismatch.
        invalid=(ProjectedPage(0,'White to move'),
                 ProjectedPage(1,'Your turn / White'),
                 ProjectedPage(0,'Your turn / Black'),
                 ProjectedPage(0,'Your turn / White',available=False),
                 ProjectedPage(0,'Your turn / White',visible=False),
                 ProjectedPage(0,'Your turn / White',connection='Read-only · unresolved'))
        for page in invalid:
            with self.subTest(page=page.__dict__), \
                 mock.patch('continuity_acceptance.rendered_canvas_pixels') as pixels:
                with self.assertRaises(BrowserTimeout):pair.board(0,'White to move',page=page)
                pixels.assert_not_called()
        with self.assertRaises(BrowserTimeout):
            pair.board(0,'White to move / CHECK',page=ProjectedPage(0,'Your turn / White'))
        page=ProjectedPage(0,'Black to move',connection='Read-only · unresolved')
        with mock.patch('continuity_acceptance.rendered_canvas_pixels',side_effect=AcceptanceFailure('no real pixels')) as pixels:
            with self.assertRaises(AcceptanceFailure):pair.board(0,'Black to move',readonly=True,page=page)
            pixels.assert_called_once_with(page)
        with self.assertRaises(AcceptanceFailure):pair.board(2,'White to move',page=page)
        with self.assertRaises(AcceptanceFailure):pair.board(0,'unknown private text',page=page)

    def test_pair_timeout_retains_closed_stage_and_attach_trace_only(self):
        pages=[mock.MagicMock(),mock.MagicMock()]
        contexts=[mock.Mock(),mock.Mock()]
        for context,page in zip(contexts,pages): context.new_page.return_value=page
        created=mock.Mock(status=200);joined=mock.Mock(status=200)
        pages[0].expect_response.return_value.__enter__.return_value=SimpleNamespace(value=created)
        pages[1].expect_response.return_value.__enter__.return_value=SimpleNamespace(value=joined)
        pages[0].wait_for_function.side_effect=BrowserTimeout('private match and credentials')
        facts=[{'account_id':'private-white'},{'account_id':'private-black'}]
        def enter(page,match_id,role,trace):
            trace.append({'expected_seat':role,'phase':'complete','typed_seat_matches':True})
        results={}
        with pair_diagnostics(results), \
             mock.patch('continuity_acceptance.enroll_actual_page'), \
             mock.patch('continuity_acceptance.context_facts',side_effect=facts), \
             mock.patch('continuity_acceptance.actual_response_json',side_effect=[
                 {'match_id':'private-match','join_code':'private-invite'}, {'seat':1}]), \
             mock.patch('continuity_acceptance.enter_game',side_effect=enter), \
             mock.patch('continuity_acceptance.rendered_canvas_pixels') as pixels:
            with self.assertRaises(BrowserTimeout):Pair(contexts,mock.Mock(),'private-audit-label')
        self.assertEqual(results,{'active_pair':{'phase':'board','role':'white','attachments':[
            {'expected_seat':0,'phase':'complete','typed_seat_matches':True}]}})
        self.assertNotIn('private',json.dumps(results));pixels.assert_not_called()
        # A later standalone helper has no retained owner from the failed run.
        from continuity_acceptance import _PAIR_RESULTS
        self.assertIsNone(_PAIR_RESULTS.get())

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

    def test_cookie_rotation_uses_the_existing_empty_body_no_content_contract(self):
        class AfterRotation(Exception):
            pass

        white, black, control = mock.Mock(), mock.Mock(), mock.Mock()
        context = mock.Mock()
        context.new_page.return_value = control
        context.cookies.side_effect = [[{'name': '__Host-tabula_session', 'value': value}]
                                      for value in ('before', 'after')]
        pair = mock.Mock(pages=[white, black],
                         attachments=[[{'operation_scope': 'same-scope'}], []],
                         facts=[{'account_id': 'same-account'}, {}])
        pair.tap.return_value = 'original'

        def session_contract(page, path, body, csrf):
            self.assertIs(page, control)
            self.assertEqual(path, '/api/v1/auth/refresh')
            self.assertEqual(body, {})
            self.assertEqual(csrf, 'current-csrf')
            return {'status': 204, 'body': None}

        with mock.patch('continuity_acceptance.Pair', return_value=pair), \
             mock.patch('continuity_acceptance.PageNetwork'), \
             mock.patch('continuity_acceptance.held'), \
             mock.patch('continuity_acceptance.fault_control', return_value={'status': 200}), \
             mock.patch('continuity_acceptance.pending_record', return_value={'operation_scope': 'same-scope'}), \
             mock.patch('continuity_acceptance.context_facts', side_effect=[
                 {'account_id': 'same-account', 'csrf_token': 'current-csrf'}, AfterRotation()]), \
             mock.patch('continuity_acceptance.api', side_effect=session_contract) as request:
            # Stop after the real harness accepts the successful session response;
            # mocks establish carrier compatibility, not rendered rotation evidence.
            with self.assertRaises(AfterRotation):
                same_record_rotation_game([context, mock.Mock()], None, None, [])
        request.assert_called_once()

    def test_focus_logout_uses_the_existing_empty_body_no_content_contract(self):
        class AfterLogout(Exception):
            pass

        white, black, popup = mock.MagicMock(), mock.Mock(), mock.Mock()
        white.locator.return_value.bounding_box.return_value = {'width': 1100, 'height': 850, 'x': 0, 'y': 0}
        white.expect_popup.return_value.__enter__.return_value = SimpleNamespace(value=popup)
        white.wait_for_function.side_effect = [None, None, None, AfterLogout()]
        white.evaluate.side_effect = [None, None, None,
                                     {'trusted': True, 'concealed': True,
                                      'selection_clear': True, 'framebuffer': 'cleared_pixels'}, True]
        pair = mock.Mock(pages=[white, black], commands=[[], []],
                         facts=[{'account_id': 'same-account'}, {}])

        def session_contract(page, path, body, csrf):
            self.assertIs(page, popup)
            self.assertEqual(path, '/api/v1/auth/logout')
            self.assertEqual(body, {})
            self.assertEqual(csrf, 'current-csrf')
            return {'status': 204, 'body': None}

        with mock.patch('continuity_acceptance.Pair', return_value=pair), \
             mock.patch('continuity_acceptance.context_facts', return_value={
                 'account_id': 'same-account', 'csrf_token': 'current-csrf'}), \
             mock.patch('continuity_acceptance.api', side_effect=session_contract) as request:
            # Native gestures and pixels are doubles here. This only guards the
            # session carrier so actual headed CI can reach its privacy oracle.
            with self.assertRaises(AfterLogout):
                focus_only_revoke_game([], None, [])
        request.assert_called_once()

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

    def test_pair_ignores_failed_header_only_attachment_and_requires_completed_body(self):
        pair=Pair.__new__(Pair);pair.match_id='a'*32;pair.attachments=[[],[]]
        response=mock.Mock(status=200,url='https://localhost:9443/api/v1/matches/'+pair.match_id+'/attach')
        request=mock.Mock(method='POST',url=response.url,failure='net::ERR_ABORTED')
        request.response.return_value=response
        response.json.side_effect=AssertionError('observer must never fall back to CDP JSON')
        response.body.side_effect=AssertionError('observer must never fall back to CDP body')
        with mock.patch('continuity_acceptance.actual_response_json') as observed:
            pair.observe_attach_finished(request,1)
            observed.assert_not_called();self.assertEqual(pair.attachments,[[],[]])
            request.failure=None;observed.return_value={'version':2,'seat':1,'frames':[]}
            pair.observe_attach_finished(request,1)
            observed.assert_called_once_with(response)
            self.assertEqual(pair.attachments[1],[observed.return_value])
            observed.return_value={'version':2,'seat':0,'frames':[]}
            with self.assertRaises(AcceptanceFailure):pair.observe_attach_finished(request,1)
            self.assertEqual(len(pair.attachments[1]),1)
            for code in ('response_json_invalid','response_body_read_failed'):
                observed.side_effect=ObservationFailure(code)
                with self.subTest(code=code),self.assertRaises(ObservationFailure) as failure:
                    pair.observe_attach_finished(request,1)
                self.assertEqual(failure.exception.code,code)
                self.assertEqual(len(pair.attachments[1]),1)
        response.json.assert_not_called();response.body.assert_not_called()

    def test_attachment_observer_rejects_wrong_seat_and_canonical_payload(self):
        pair=Pair.__new__(Pair);pair.match_id='a'*32;pair.attachments=[[],[]]
        response=mock.Mock(status=200,url='https://localhost:9443/api/v1/matches/'+pair.match_id+'/attach')
        with mock.patch('continuity_acceptance.actual_response_json') as observed:
            observed.return_value={'version':2,'seat':1}
            with self.assertRaises(AcceptanceFailure):pair.observe_attach(response,0)
            observed.return_value={'version':2,'seat':0,'seed':[1]}
            with self.assertRaises(AcceptanceFailure):pair.observe_attach(response,0)
            self.assertEqual(pair.attachments,[[],[]])
            observed.return_value={'version':2,'seat':0,'frames':[]}
            pair.observe_attach(response,0);self.assertEqual(len(pair.attachments[0]),1)

if __name__=='__main__':unittest.main()
