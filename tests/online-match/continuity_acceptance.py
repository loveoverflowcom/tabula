#!/usr/bin/env python3
"""Actual rendered PR3 interruptions through Chromium, HTTPS and real PostgreSQL.

Only deliberate browser network disconnection/abort and real child SIGKILL
inject failures. No mocked successful response, replacement projection, copied
credentials, ignored TLS error, local tunnel, HAR, or private payload artifact.
"""
from __future__ import annotations
import argparse
import http.client
import ssl
import json
import os
from pathlib import Path
import time
from urllib.parse import urlsplit
from playwright.sync_api import sync_playwright
from browser_acceptance import (AcceptanceFailure, GAME_PATH, MATCH_VERSION, ORIGIN,
    SESSION_COOKIE, TERMINAL_STATUS, api, board_square, context_facts, denied,
    enroll_actual_page, enter_game, exception_class, move, private_frame_keys,
    rendered_canvas_pixels, require, setup_browser_trust, wire_probe, start_native_poll, publication_control, reattach_required, completed_attachment_response)
from capture_evidence import CaptureEvidence
from actual_response import actual_response_json, install_actual_response_observer
from process_supervisor import SupervisorClient

RECOVERING_CONCEALED = """() => {
 const root=document.documentElement, canvas=document.querySelector('#glcanvas');
 const spans=document.querySelectorAll('#online-status-container span');
 return root.dataset.onlineAvailability==='recovering'
  && ['onlineSeat','onlineRevision','onlineStatus'].every(key=>!Object.hasOwn(root.dataset,key))
  && canvas && canvas.getAttribute('aria-hidden')==='true'
  && getComputedStyle(canvas).visibility==='hidden'
  && Array.from(spans).every(node=>node.textContent.trim()==='');
}"""
CURRENT_BOARD = """({seat,status,readonly}) => {
 const d=document.documentElement.dataset, c=document.querySelector('#glcanvas');
 return d.onlineAvailability==='available' && d.onlineSeat===String(seat)
  && d.onlineStatus===status && c && !c.hidden && !c.hasAttribute('aria-hidden') && getComputedStyle(c).visibility==='visible' && c.getBoundingClientRect().width>0
  && (readonly ? d.onlineConnection?.startsWith('Read-only') : d.onlineConnection==='Connected · server-authoritative');
}"""
MOVES = [('f2','f3'),('e7','e5'),('g2','g4'),('d8','h4')]


def fault_control(ca: Path, operation: str, token: str) -> dict:
    require(operation in ('status','release'), 'unexpected fixture fault operation')
    return wire_probe(ca, '/__fixture/continuity/'+operation,
                      [('Origin',ORIGIN),('Content-Type','application/json'),
                       ('X-Tabula-Fixture-Control',token)], '{"version":1}')


def held(ca: Path, token: str) -> None:
    deadline=time.monotonic()+15
    while time.monotonic()<deadline:
        result=fault_control(ca,'status',token)
        require(result['status']==200, 'actual command fault gate unavailable')
        phase=result['body']['phase']
        if phase=='held': return
        require(phase=='armed', 'actual command failed before requested fault boundary')
        time.sleep(.025)
    raise AcceptanceFailure('actual command never reached requested fault boundary')


def command_identity(body: str) -> dict:
    value=json.loads(body)
    require(value.get('version')==MATCH_VERSION and isinstance(value.get('command'),dict),
            'real browser command identity absent')
    require(not private_frame_keys(value), 'real command carried canonical facts')
    return value['command']


def pending_record(page, match_id: str) -> dict | None:
    raw=page.evaluate('key => sessionStorage.getItem(key)', 'tabula.pending.v2.'+match_id)
    if raw is None: return None
    require(len(raw)<=133120,'pending browser hint exceeded bounds')
    value=json.loads(raw)
    require(set(value)<=set(('version','match_id','game_id','game_version','operation_scope','command','expires_at','unknown')),
            'browser persisted unauthorized recovery data')
    require(not private_frame_keys(value),'browser persisted canonical recovery data')
    require(value.get('match_id')==match_id and value.get('version')==MATCH_VERSION,
            'browser pending hint changed its match binding')
    return value


class PageNetwork:
    """A page-scoped real Chromium network failure; other pages can still act."""
    def __init__(self,page):
        self.page=page;self.session=page.context.new_cdp_session(page)
        self.session.send('Network.enable')
    def offline(self,value: bool):
        self.session.send('Network.emulateNetworkConditions',
                          {'offline':value,'latency':0,'downloadThroughput':-1,'uploadThroughput':-1})
    def close(self):
        self.offline(False);self.session.detach()


class Pair:
    def __init__(self,contexts,private:Path,label:str):
        self.pages=[context.new_page() for context in contexts]
        self.contexts=contexts;self.private=private;self.label=label
        self.attachments=[[],[]];self.commands=[[],[]]
        self.facts=[];self.match_id=None;self.audit_inputs=0;self.expected_scopes=2
        for role,page in enumerate(self.pages):
            page.on('requestfinished',lambda request,r=role:self.observe_attach_finished(request,r))
            page.on('request',lambda request,r=role:self.observe_command(request,r))
            page.goto(ORIGIN+'/__fixture/enroll',wait_until='domcontentloaded')
            enroll_actual_page(page,('white','black')[role],{'startup':{'enrollment':[]}})
            self.facts.append(context_facts(page))
        require(self.facts[0]['account_id']!=self.facts[1]['account_id'],'fault opponents share an account')
        white,black=self.pages
        with white.expect_response(lambda r:urlsplit(r.url).path=='/api/v1/matches') as created:
            white.get_by_test_id('online-create').click()
        require(created.value.status==200,'fault match creation failed')
        admitted=actual_response_json(created.value);self.match_id=admitted['match_id']
        black.get_by_test_id('online-join-code').fill(admitted['join_code'])
        with black.expect_response(lambda r:urlsplit(r.url).path=='/api/v1/matches/join') as joined:
            black.get_by_test_id('online-join').click()
        require(joined.value.status==200 and actual_response_json(joined.value)['seat']==1,'fault opponent admission failed')
        for role,page in enumerate(self.pages):
            enter_game(page,self.match_id,role)
            self.board(role,'White to move')
        self.write_audit(0)
    def observe_attach_finished(self,request,role):
        if self.match_id:
            response=completed_attachment_response(request,self.match_id)
            if response is not None:self.observe_attach(response,role)
    def observe_attach(self,response,role):
        path=urlsplit(response.url).path
        if self.match_id and path==f'/api/v1/matches/{self.match_id}/attach' and response.status==200:
            value=actual_response_json(response)
            require(value['seat']==role and value['version']==MATCH_VERSION,'recovery changed the server-owned seat')
            require(not private_frame_keys(value),'recovery attachment leaked canonical facts')
            self.attachments[role].append(value)
    def observe_command(self,request,role):
        if self.match_id and urlsplit(request.url).path==f'/api/v1/matches/{self.match_id}/command':
            require(request.post_data is not None,'actual interrupted command body missing')
            self.commands[role].append(request.post_data)
    def board(self,role,status,readonly=False,page=None):
        page=page or self.pages[role]
        page.bring_to_front()
        page.wait_for_function(CURRENT_BOARD,arg={'seat':role,'status':status,'readonly':readonly},timeout=60_000)
        rendered_canvas_pixels(page)
    def arm(self,role,point):
        self.facts[role]=context_facts(self.pages[role])
        reply=api(self.pages[role],'/__fixture/continuity/arm',
                  {'version':1,'match_id':self.match_id,'attachment_id':self.attachments[role][-1]['attachment_id'],'point':point},self.facts[role]['csrf_token'])
        require(reply['status']==200,'actual native command barrier could not be armed')
        return reply['body']['control_token']
    def tap(self,role,index,page=None):
        page=page or self.pages[role];canvas=page.locator('#glcanvas');bounds=canvas.bounding_box()
        require(bounds is not None,'actual fault board has no pointer geometry')
        with page.expect_request(lambda r:urlsplit(r.url).path==f'/api/v1/matches/{self.match_id}/command') as requested:
            for square in MOVES[index]:
                x,y=board_square(bounds['width'],bounds['height']-56,square,False)
                canvas.click(position={'x':x,'y':y},delay=70)
        require(requested.value.post_data is not None,'actual pointer command bytes missing')
        return requested.value.post_data
    def oracle(self,expected,role=1):
        self.facts[role]=context_facts(self.pages[role])
        reply=api(self.pages[role],'/__fixture/continuity/oracle',{'version':1,'match_id':self.match_id,'expected_inputs':expected},self.facts[role]['csrf_token'])
        require(reply['status']==200 and reply['body']['expected_public_transcript_prefix'] is True,
                'independent committed prefix differs from expected public moves')
        self.write_audit(expected)
    def write_audit(self,expected):
        self.audit_inputs=expected
        (self.private/('continuity-audit-'+self.label+'.json')).write_text(json.dumps(
            {'match_id':self.match_id,'accounts':[f['account_id'] for f in self.facts],'expected_inputs':expected,'expected_scopes':self.expected_scopes}))
    def full(self,start=0,white=None):
        for index in range(start,4):
            role=index%2;page=white if role==0 and white is not None else self.pages[role]
            move(page,*MOVES[index],False,self.match_id)
            self.write_audit(index+1)
            status=TERMINAL_STATUS if index==3 else ('Black to move' if role==0 else 'White to move')
            self.board(1-role,status,page=(white if 1-role==0 and white is not None else None))
        self.board(1,TERMINAL_STATUS);self.oracle(4)
    def close(self):
        for context in self.contexts:
            for page in list(context.pages): page.close()
            context.clear_cookies()


def restored_same_scope(pair:Pair,role:int,old:dict,original:str):
    pair.pages[role].wait_for_function('key => sessionStorage.getItem(key) === null',arg='tabula.pending.v2.'+pair.match_id,timeout=30_000)
    new=pair.attachments[role][-1]
    require(new['attachment_id']!=old['attachment_id'] and new['operation_scope']==old['operation_scope'],
            'recovery changed operation identity or retained the retired transport')
    require(any(command_identity(body)==command_identity(original) for body in pair.commands[role][1:]),
            'recovery did not retry the exact original sequence and payload')
    pair.facts[role]=context_facts(pair.pages[role])
    reattach_required(api(pair.pages[role],f'/api/v1/matches/{pair.match_id}/poll',{'version':MATCH_VERSION,'attachment_id':old['attachment_id']},pair.facts[role]['csrf_token']),'retired attachment restored queued output')
    reattach_required(api(pair.pages[role],f'/api/v1/matches/{pair.match_id}/command',original,pair.facts[role]['csrf_token']),'retired attachment admitted the original command')


def ready_server(page):
    deadline=time.monotonic()+20
    while time.monotonic()<deadline:
        try:
            page.goto(ORIGIN+'/__fixture/enroll',wait_until='domcontentloaded',timeout=2000)
            if page.get_by_test_id('fixture-enroll').is_visible():return
        except Exception:pass
        time.sleep(.1)
    raise AcceptanceFailure('restarted real native server did not answer validated HTTPS')


def network_refresh_game(contexts,private,ca,results,evidence):
    pair=Pair(contexts,private,'network-refresh');white,black=pair.pages
    network=PageNetwork(white);old=pair.attachments[0][-1]
    path=f'**/api/v1/matches/{pair.match_id}/command'
    def before_send(route):network.offline(True);route.abort('internetdisconnected')
    white.route(path,before_send,times=1)
    original=pair.tap(0,0)
    white.wait_for_function(RECOVERING_CONCEALED,timeout=10_000)
    require(pending_record(white,pair.match_id) is not None,'uncertain send lost its pending operation hint')
    pair.oracle(0)
    evidence.capture(white,'11-pending-network-loss.png','Actual pending move concealed during network loss','White browser','Actual pointer command aborted before send; authorized canvas and projected status concealed pending fresh authority')
    network.offline(False)
    # An actual document refresh must restore only identity/payload hints, then
    # reacquire context/grant/scope before replay; it never restores view bytes.
    white.reload(wait_until='domcontentloaded')
    pair.board(0,'Black to move');pair.board(1,'Black to move');pair.oracle(1)
    restored_same_scope(pair,0,old,original)
    results.append({'case':'drop_before_send_refresh_pending','pass':True,'committed_once':True,'exact_retry':True,'concealed_before_fresh_authority':True})
    # Real SQL writes are staged while the request connection drops. The server
    # owns commit resolution; client cancellation cannot decide the outcome.
    old=pair.attachments[1][-1];gate=pair.arm(1,'staged_commit');black_network=PageNetwork(black)
    original=pair.tap(1,1);held(ca,gate);pair.oracle(1,0)
    black_network.offline(True)
    released=fault_control(ca,'release',gate)
    require(released['status']==200,'staged actual COMMIT gate could not release')
    black.wait_for_function(RECOVERING_CONCEALED,timeout=10_000)
    pair.board(0,'White to move');pair.oracle(2,0)
    black_network.offline(False);pair.board(1,'White to move');restored_same_scope(pair,1,old,original)
    results.append({'case':'drop_during_staged_apply_commit','pass':True,'committed_once':True,'exact_retry':True,'timeout_meant_failed_commit':False})
    move(white,*MOVES[2],False,pair.match_id);pair.write_audit(3);pair.board(1,'Black to move')
    # Drop actual poll transport while the other process commits the final move.
    def drop_poll(route):network.offline(True);route.abort('internetdisconnected')
    white.route(f'**/api/v1/matches/{pair.match_id}/poll',drop_poll,times=1)
    white.wait_for_function(RECOVERING_CONCEALED,timeout=15_000)
    move(black,*MOVES[3],False,pair.match_id);pair.write_audit(4);pair.board(1,TERMINAL_STATUS)
    network.offline(False);pair.board(0,TERMINAL_STATUS);pair.oracle(4)
    require(pending_record(white,pair.match_id) is None,'settled commands retained a pending hint')
    white.reload(wait_until='domcontentloaded');pair.board(0,TERMINAL_STATUS)
    require(any(frame.get('body',{}).get('MatchUpdate',{}).get('revision')==0 and frame.get('body',{}).get('MatchUpdate',{}).get('view') for frame in pair.attachments[0][-1]['frames']),'committed refresh did not perform full projection resync')
    results.append({'case':'lost_output_poll_refresh_committed','pass':True,'fresh_full_projection':True,'stable_seats':True})
    evidence.capture(white,'12-refreshed-terminal-board.png','Actual terminal board after interrupted polls and refresh','White browser','Fresh authorized full projection after four durable moves; same exact Black checkmate verdict',canvas=True)
    network.close();black_network.close();pair.close()


def crash_game(contexts,private,ca,supervisor,committed,results):
    label='committed-crash' if committed else 'uncommitted-crash'
    pair=Pair(contexts,private,label);white,black=pair.pages;old=pair.attachments[0][-1]
    gate=pair.arm(0,'after_commit' if committed else 'staged_commit')
    original=pair.tap(0,0);held(ca,gate)
    # A confirmed phase, not elapsed time, selects the committed/uncommitted
    # partition. The independent second process sees durable truth after restart.
    pair.oracle(1 if committed else 0,1)
    white_network=PageNetwork(white);white_network.offline(True)
    white.wait_for_function(RECOVERING_CONCEALED,timeout=10_000)
    outcome=supervisor.kill();require(outcome['sigkill_reaped'] is True,'actual native server SIGKILL was not reaped')
    restarted=supervisor.restart('normal');require(restarted['alive'] is True,'native server restart failed')
    pair.board(1,'Black to move' if committed else 'White to move')
    pair.oracle(1 if committed else 0,1)
    white_network.offline(False);pair.board(0,'Black to move');pair.oracle(1)
    restored_same_scope(pair,0,old,original)
    pair.full(1)
    results.append({'case':label,'pass':True,'actual_sigkill_reaped':True,'fresh_fenced_owner':True,'observed_committed_partition':committed,'exact_retry_committed_once':True,'stable_seats':True})
    white_network.close();pair.close()


def apply_and_committed_refresh_game(contexts,private,ca,point,results):
    label='pure-apply-loss' if point=='before_commit' else 'committed-refresh'
    pair=Pair(contexts,private,label);white,black=pair.pages;old=pair.attachments[0][-1]
    gate=pair.arm(0,point);original=pair.tap(0,0);held(ca,gate)
    pair.oracle(0 if point=='before_commit' else 1)
    network=PageNetwork(white);network.offline(True)
    require(fault_control(ca,'release',gate)['status']==200,'actual apply/receipt barrier could not release')
    white.wait_for_function(RECOVERING_CONCEALED,timeout=10_000)
    require(pending_record(white,pair.match_id) is not None,'unknown committed/apply request lost its exact pending identity')
    pair.board(1,'Black to move');pair.oracle(1)
    network.offline(False)
    if point=='after_commit':white.reload(wait_until='domcontentloaded')
    pair.board(0,'Black to move');restored_same_scope(pair,0,old,original)
    pair.full(1)
    results.append({'case':'drop_after_pure_apply_before_append' if point=='before_commit' else 'refresh_committed_before_original_ack','pass':True,'known_gate_partition':True,'exact_original_retry':True,'no_duplicate_move':True,'fresh_authority_before_render':True})
    network.close();pair.close()


def same_record_rotation_game(contexts,private,ca,results):
    pair=Pair(contexts,private,'same-record-rotation');white,black=pair.pages
    old=pair.attachments[0][-1];gate=pair.arm(0,'after_commit')
    original=pair.tap(0,0);held(ca,gate);pair.oracle(1)
    network=PageNetwork(white);network.offline(True)
    require(fault_control(ca,'release',gate)['status']==200,'committed rotation-case barrier could not release')
    white.wait_for_function(RECOVERING_CONCEALED,timeout=10_000)
    pending=pending_record(white,pair.match_id)
    require(pending is not None and pending['operation_scope']==old['operation_scope'],
            'uncertain original operation was not retained before rotation')
    pair.board(1,'Black to move')
    control=contexts[0].new_page();control.goto(ORIGIN+GAME_PATH,wait_until='domcontentloaded')
    before_cookie=next(cookie['value'] for cookie in contexts[0].cookies() if cookie['name']==SESSION_COOKIE)
    current=context_facts(control)
    require(current['account_id']==pair.facts[0]['account_id'],'rotation control page changed the account')
    refreshed=api(control,'/api/v1/auth/refresh',{},current['csrf_token'])
    require(refreshed['status']==204 and refreshed['body'] is None,
            'existing browser credential rotation did not complete')
    after_cookie=next(cookie['value'] for cookie in contexts[0].cookies() if cookie['name']==SESSION_COOKIE)
    require(before_cookie!=after_cookie,'browser refresh did not rotate its HttpOnly credential')
    current=context_facts(control)
    require(current['account_id']==pair.facts[0]['account_id'],'credential rotation changed the account')
    # Black's legal command prepares its real fan-out. White's cached old digest
    # can no longer authorize that output, so its local attachment is retired.
    move(black,*MOVES[1],False,pair.match_id);pair.board(1,'White to move');pair.oracle(2)
    stale=api(control,f'/api/v1/matches/{pair.match_id}/poll',
              {'version':MATCH_VERSION,'attachment_id':old['attachment_id']},current['csrf_token'])
    reattach_required(stale,'valid rotated membership received stale-attachment output')
    observed={'original_ack':False}
    def original_ack(response):
        if urlsplit(response.url).path!=f'/api/v1/matches/{pair.match_id}/command' or response.status!=200:return
        body=response.request.post_data
        if body is None or command_identity(body)!=command_identity(original):return
        outer=json.loads(body)
        if outer['attachment_id']==old['attachment_id']:return
        frames=actual_response_json(response)['frames']
        require(not private_frame_keys(frames),'rotated original receipt leaked canonical facts')
        observed['original_ack']=any(frame.get('body',{}).get('Ack',{}).get('seq')==command_identity(original)['seq'] for frame in frames)
    white.on('response',original_ack)
    try:
        control.close();network.offline(False)
        pair.board(0,'White to move');restored_same_scope(pair,0,old,original)
        require(observed['original_ack'],'same-record rotation did not reproduce the exact original Ack')
        pair.oracle(2);pair.full(2)
    finally:white.remove_listener('response',original_ack)
    results.append({'case':'same_auth_record_rotation_preserves_uncertain_original_operation',
                    'pass':True,'credential_rotated':True,'reattach_required_observed':True,
                    'same_operation_scope':True,'exact_original_ack':True,'no_duplicate_move':True})
    network.close();pair.close()


def current_context_after_restart(ca:Path,cookie:str,account:str)->dict:
    """Real CA-validated context read; credentials and CSRF stay in memory."""
    require(os.environ.get('CI') in ('true','1') and os.environ.get('TABULA_ONLINE_MATCH_DISPOSABLE')=='1',
            'restart context oracle requires disposable CI')
    deadline=time.monotonic()+15
    while time.monotonic()<deadline:
        connection=http.client.HTTPSConnection('localhost',9443,timeout=2,
                        context=ssl.create_default_context(cafile=str(ca)))
        try:
            connection.request('GET','/api/v1/auth/context',headers={'Cookie':SESSION_COOKIE+'='+cookie,'Accept':'application/json','Cache-Control':'no-store'})
            response=connection.getresponse();raw=response.read(65537)
            require(len(raw)<=65536,'restart context exceeded its response bound')
            if response.status in (502,503):
                time.sleep(.05);continue
            require(response.status==200 and response.getheader('Content-Type','').split(';')[0]=='application/json'
                    and 'no-store' in response.getheader('Cache-Control','').split(','),'restarted authority context was not validated')
            value=json.loads(raw)
            require(value.get('version')==1 and value.get('disposition')=='authenticated' and value.get('account_id')==account
                    and isinstance(value.get('csrf_token'),str) and len(value['csrf_token'])==43,'restart changed current account authority')
            require(not private_frame_keys(value),'restart context exposed canonical facts')
            return value
        except ssl.SSLCertVerificationError:
            raise AcceptanceFailure('restart context TLS verification failed') from None
        except (OSError,http.client.HTTPException):time.sleep(.05)
        finally:connection.close()
    raise AcceptanceFailure('restarted native authority did not become ready')


def restart_between_grant_and_attach_game(contexts,private,ca,supervisor,results):
    pair=Pair(contexts,private,'restart-grant-attach');white,black=pair.pages
    old=pair.attachments[0][-1];gate=pair.arm(0,'after_commit')
    original=pair.tap(0,0);held(ca,gate);pair.oracle(1)
    network=PageNetwork(white);network.offline(True)
    require(fault_control(ca,'release',gate)['status']==200,'grant-restart committed barrier could not release')
    white.wait_for_function(RECOVERING_CONCEALED,timeout=10_000)
    require(pending_record(white,pair.match_id) is not None,'grant restart lost its uncertain original operation')
    pair.board(1,'Black to move');pair.oracle(1)
    cookie=next(entry['value'] for entry in contexts[0].cookies() if entry['name']==SESSION_COOKIE)
    observed={'held_real_attach':False,'actual_sigkill':False,'old_csrf_403':False,'old_grant_409':False,'fresh_attach':False,'original_ack':False}
    def hold_real_attach(route):
        # Keep the real browser's exact body and headers unchanged. The grant
        # and CSRF belong to the process that is genuinely killed here.
        request=route.request;body=request.post_data
        require(body is not None,'held real grant-bound attach body absent')
        observed['held_real_attach']=True
        killed=supervisor.kill();require(killed['sigkill_reaped'] is True,'grant boundary native SIGKILL was not reaped')
        observed['actual_sigkill']=True
        require(supervisor.restart('normal')['alive'] is True,'grant boundary server restart failed')
        current=current_context_after_restart(ca,cookie,pair.facts[0]['account_id'])
        # This separate current-CSRF probe establishes only the old signed-grant
        # boundary. It cannot replace the unchanged browser request below.
        denied_grant=wire_probe(ca,f'/api/v1/matches/{pair.match_id}/attach',
                    [('Origin',ORIGIN),('Content-Type','application/json'),('Cookie',SESSION_COOKIE+'='+cookie),('X-Tabula-CSRF',current['csrf_token'])],body)
        denied(denied_grant,{409},'old signed grant authorized an attachment after restart')
        require(denied_grant['body'].get('code')=='fresh_grant_required','old signed grant lacked its exact recoverable boundary')
        observed['old_grant_409']=True
        route.continue_()
    def observed_attach(response):
        if urlsplit(response.url).path!=f'/api/v1/matches/{pair.match_id}/attach':return
        if response.status==403:
            value=actual_response_json(response);denied({'status':403,'body':value},{403},'old CSRF attach released projection frames')
            require(value.get('code')=='request_rejected','unchanged restarted attach did not hit the real CSRF boundary')
            observed['old_csrf_403']=True
        elif response.status==200:
            require(observed['old_csrf_403'],'fresh attach bypassed the required unchanged old-CSRF request')
            observed['fresh_attach']=True
    def observed_ack(response):
        if urlsplit(response.url).path!=f'/api/v1/matches/{pair.match_id}/command' or response.status!=200:return
        body=response.request.post_data
        if body is None or command_identity(body)!=command_identity(original) or json.loads(body)['attachment_id']==old['attachment_id']:return
        frames=actual_response_json(response)['frames'];require(not private_frame_keys(frames),'restarted original Ack exposed canonical facts')
        observed['original_ack']=any(frame.get('body',{}).get('Ack',{}).get('seq')==command_identity(original)['seq'] for frame in frames)
    white.route(f'**/api/v1/matches/{pair.match_id}/attach',hold_real_attach,times=1)
    white.on('response',observed_attach);white.on('response',observed_ack)
    try:
        network.offline(False);pair.board(0,'Black to move');pair.board(1,'Black to move')
        restored_same_scope(pair,0,old,original)
        require(all(observed.values()),'grant/CSRF restart did not exercise every real recovery boundary')
        pair.oracle(1);pair.full(1)
    finally:
        white.remove_listener('response',observed_attach);white.remove_listener('response',observed_ack)
    results.append({'case':'restart_between_grant_and_attach_recovers_unchanged_old_csrf','pass':True,
                    'actual_sigkill_reaped':True,'unchanged_request_403_without_frames':True,'fresh_context_grant_scope_before_restore':True,'exact_original_ack':True,'no_duplicate_move':True})
    results.append({'case':'old_signed_grant_with_fresh_current_csrf_requires_fresh_grant','pass':True,
                    'separate_current_csrf_probe':True,'exact_409_fresh_grant_required':True,'no_attachment_projection_or_apply_from_old_grant':True})
    network.close();pair.close()


def changed_identity_game(contexts,private,ca,new_record,results):
    label='new-record-scope' if new_record else 'cross-account-scope'
    pair=Pair(contexts,private,label);white,black=pair.pages
    gate=pair.arm(0,'after_commit');original=pair.tap(0,0);held(ca,gate)
    network=PageNetwork(white);network.offline(True)
    require(fault_control(ca,'release',gate)['status']==200,'identity fault committed command barrier failed')
    white.wait_for_function(RECOVERING_CONCEALED,timeout=10_000);pair.board(1,'Black to move');pair.oracle(1)
    require(pending_record(white,pair.match_id) is not None,'identity change lost unknown original command prematurely')
    control=contexts[0].new_page();control.goto(ORIGIN+GAME_PATH,wait_until='domcontentloaded')
    if new_record:
        changed=api(control,'/__fixture/continuity/new-record',{'version':1},pair.facts[0]['csrf_token'])
        require(changed['status']==200 and changed['body']['new_current_record_issued'] is True,'same-account new current auth record was not issued')
        facts=context_facts(control);require(facts['account_id']==pair.facts[0]['account_id'],'new auth record changed the intended account')
        pair.expected_scopes=3
    else:
        changed=api(control,'/__fixture/continuity/authority',{'version':1,'match_id':pair.match_id,'change':'revoke'},pair.facts[0]['csrf_token'])
        require(changed['status']==200,'old account session was not revoked before switch')
        contexts[0].clear_cookies()
        control.goto(ORIGIN+'/__fixture/enroll',wait_until='domcontentloaded')
        enroll_actual_page(control,'white',{'startup':{'enrollment':[]}})
        require(context_facts(control)['account_id']!=pair.facts[0]['account_id'],'actual account switch did not change the account')
    control.close();count=len(pair.commands[0]);network.offline(False)
    white.reload(wait_until='domcontentloaded')
    if new_record:
        pair.board(0,'Black to move',readonly=True)
        require(pair.attachments[0][-1]['operation_scope']!=json.loads(original).get('operation_scope',pair.attachments[0][0]['operation_scope']),'new current auth record retained the original operation scope')
        value=pending_record(white,pair.match_id);require(value is not None and value.get('unknown') is True,'scope change did not preserve an explicit unknown result')
    else:
        white.wait_for_function("() => document.documentElement.dataset.onlineAvailability==='unavailable' && !Object.hasOwn(document.documentElement.dataset,'onlineStatus') && document.querySelector('#glcanvas').hidden",timeout=30_000)
    white.wait_for_timeout(350)
    require(len(pair.commands[0])==count,'old pending command was replayed across current identity scope')
    pair.write_audit(1);pair.oracle(1)
    results.append({'case':'new_auth_record_never_replays_old_operation' if new_record else 'cross_account_never_restores_old_projection_or_replays','pass':True,'no_old_scope_replay':True,'no_duplicate_move':True,'same_account_new_scope':new_record,'unauthorized_old_view_concealed':not new_record})
    network.close();pair.close()


def repeated_recovery_and_navigation(contexts,private,results,optional):
    pair=Pair(contexts,private,'repeat-navigation');white,black=pair.pages
    network=PageNetwork(white)
    for cycle in range(2):
        def drop_poll(route):network.offline(True);route.abort('internetdisconnected')
        white.route(f'**/api/v1/matches/{pair.match_id}/poll',drop_poll,times=1)
        white.wait_for_function(RECOVERING_CONCEALED,timeout=15_000)
        # Let the genuine bounded reconnect attempts exhaust. An explicit Retry
        # starts a fresh document; it must not resurrect the retired grant/view.
        white.locator('#runtime-error').wait_for(state='visible',timeout=60_000)
        require(not white.locator('#glcanvas').is_visible(),'exhausted recovery retained an authorized board')
        network.offline(False)
        with white.expect_navigation(wait_until='domcontentloaded'):
            white.locator('#retry').dblclick(delay=10)
        pair.board(0,'White to move');pair.oracle(0)
    require(not pair.commands[0],'repeated recovery/Retry invented a command')
    results.append({'case':'repeated_recovery_explicit_retry','pass':True,'bounded_reconnect_exhausted_twice':True,'no_invented_command':True,'fresh_authority_before_render':True})
    old=pair.attachments[0][-1]
    white.locator('#leave').click();white.locator('#leave-dialog').wait_for(state='visible')
    white.locator('#stay').click();pair.board(0,'White to move')
    require(pair.attachments[0][-1]['attachment_id']==old['attachment_id'],'canceled leave retired an active attachment')
    # Real browser beforeunload cancellation is distinct from the app dialog.
    with white.expect_event('dialog',timeout=10_000) as warned:
        white.evaluate('history.back()')
    require(warned.value.type=='beforeunload','browser Back cancellation did not exercise beforeunload')
    warned.value.dismiss();pair.board(0,'White to move')
    require(pair.attachments[0][-1]['attachment_id']==old['attachment_id'],'canceled browser Back retired the live board')
    persisted=[]
    white.expose_function('__tabulaAcceptancePageShow',lambda value:persisted.append(value))
    observer="""addEventListener('pageshow',event=>{
        const d=document.documentElement.dataset,c=document.querySelector('#glcanvas');
        const spans=document.querySelectorAll('#online-status-container span');
        const concealed=!!c && c.width===0 && c.height===0
          && c.getAttribute('aria-hidden')==='true' && getComputedStyle(c).visibility==='hidden'
          && ['onlineSeat','onlineRevision','onlineStatus','onlineConnection'].every(key=>!Object.hasOwn(d,key))
          && Array.from(spans).every(node=>node.textContent.trim()==='');
        Promise.resolve(window.__tabulaAcceptancePageShow({persisted:event.persisted,trusted:event.isTrusted,concealed})).catch(()=>{});
    },{capture:true});"""
    white.add_init_script(observer)
    # Install the same purely observational listener in the current document;
    # it does not fabricate a lifecycle event or change application state.
    white.evaluate(observer)
    white.locator('#leave').click();white.locator('#leave-dialog').wait_for(state='visible')
    with white.expect_navigation(wait_until='domcontentloaded'):white.locator('#confirm-leave').click()
    require(urlsplit(white.url).path==GAME_PATH,'confirmed leave did not return to the authorized shell route')
    white.go_back(wait_until='domcontentloaded');pair.board(0,'White to move')
    fresh=pair.attachments[0][-1]
    require(fresh['attachment_id']!=old['attachment_id'] and fresh['operation_scope']==old['operation_scope'],'actual Back restored an old attachment or changed its operation scope')
    updates=[frame['body']['MatchUpdate'] for frame in fresh['frames'] if 'MatchUpdate' in frame.get('body',{})]
    require(any(update['revision']==0 and update['view'] for update in updates),'actual Back lacked a fresh full authorized projection')
    restored=[value for value in persisted if value.get('persisted') is True]
    require(all(value.get('trusted') is True and value.get('concealed') is True for value in restored),
            'actual BFCache restored private pixels or accessibility before fresh authority')
    optional['actual_bfcache']={'status':'pass' if restored else 'blocked','persisted_pageshow_observed':bool(restored),'first_restored_surface_concealed_before_bootstrap':bool(restored),'fresh_attach_after_return':True,'blocker':None if restored else 'actual_chromium_did_not_restore_online_document_from_bfcache'}
    pair.full()
    results.append({'case':'cancel_leave_cancel_back_then_fresh_history_return','pass':True,'canceled_leave_retained_attachment':True,'canceled_beforeunload_retained_attachment':True,'confirmed_leave_retired_attachment':True,'actual_history_return_fresh_authority':True})
    network.close();pair.close()


def retired_receipt_game(contexts,private,ca,supervisor,mode,results):
    supervisor.kill();supervisor.restart(mode)
    ready=contexts[0].new_page();ready_server(ready);ready.close()
    pair=Pair(contexts,private,mode+'-receipt');white,black=pair.pages
    gate=pair.arm(0,'after_commit');original=pair.tap(0,0);held(ca,gate)
    network=PageNetwork(white);network.offline(True)
    require(fault_control(ca,'release',gate)['status']==200,'receipt fault gate could not release')
    white.wait_for_function(RECOVERING_CONCEALED,timeout=10_000)
    require(pending_record(white,pair.match_id) is not None,'lost original Ack discarded pending command')
    pair.board(1,'Black to move');pair.write_audit(1)
    replacement=contexts[0].new_page()
    replacement.on('requestfinished',lambda request:pair.observe_attach_finished(request,0))
    replacement.goto(white.url,wait_until='domcontentloaded')
    pair.board(0,'Black to move',page=replacement)
    # Both replacement pages use the same independently issued account; no
    # credential/storage state is copied between opponent browser processes.
    pair.full(1,white=replacement)
    if mode=='expired':time.sleep(1.25)
    network.offline(False);pair.board(0,TERMINAL_STATUS,readonly=True)
    unknown=pending_record(white,pair.match_id)
    require(unknown is not None and unknown.get('unknown') is True and unknown.get('command') is None,
            'retired original receipt was mistaken for a failed command')
    require(any(command_identity(body)==command_identity(original) for body in pair.commands[0][1:]),'original uncertain command was not checked using its exact identity')
    before=len(pair.commands[0])
    canvas=white.locator('#glcanvas');bounds=canvas.bounding_box();require(bounds is not None,'resynced read-only board missing')
    for square in MOVES[0]:
        x,y=board_square(bounds['width'],bounds['height']-56,square,False);canvas.click(position={'x':x,'y':y})
    white.wait_for_timeout(350)
    require(len(pair.commands[0])==before,'unknown-result board issued a new or repeated move')
    pair.oracle(4)
    results.append({'case':mode+'_original_receipt_unknown_full_resync','pass':True,'unknown_never_failed':True,'read_only':True,'no_duplicate_move':True,'exact_retry_checked':True})
    network.close();pair.close()


def authority_before_commit(contexts,private,ca,supervisor,change,results):
    pair=Pair(contexts,private,'authority-'+change);white,black=pair.pages
    old=pair.attachments[0][-1];gate=pair.arm(0,'before_submission');pair.tap(0,0);held(ca,gate)
    changed=api(white,'/__fixture/continuity/authority',{'version':1,'match_id':pair.match_id,'change':change},pair.facts[0]['csrf_token'])
    require(changed['status']==200 and changed['body']['authority_change_committed'] is True,'current durable authority fault failed')
    require(fault_control(ca,'release',gate)['status']==200,'authority fault command barrier could not release')
    white.wait_for_function("() => document.documentElement.dataset.onlineAvailability==='unavailable' && !Object.hasOwn(document.documentElement.dataset,'onlineStatus') && document.querySelector('#glcanvas').hidden",timeout=30_000)
    pair.oracle(0)
    denied(api(white,f'/api/v1/matches/{pair.match_id}/poll',{'version':MATCH_VERSION,'attachment_id':old['attachment_id']},pair.facts[0]['csrf_token']),{401,403},'retired current authority delivered restored frames')
    results.append({'case':change+'_ordered_before_command_commit','pass':True,'no_commit':True,'no_restored_projection':True,'no_wrong_seat':True})
    pair.close()


def authority_expiry_during_commit(contexts,private,ca,results):
    pair=Pair(contexts,private,'expiry-during-commit');white,black=pair.pages
    gate=pair.arm(0,'staged_commit')
    limited=api(white,'/__fixture/continuity/authority',{'version':1,'match_id':pair.match_id,'change':'expire_soon'},pair.facts[0]['csrf_token'])
    require(limited['status']==200,'real short remaining authority deadline could not be established')
    pair.tap(0,0);held(ca,gate)
    # Actual database wall time crosses both the original current-session idle
    # deadline and the bounded transaction deadline while SQL remains staged.
    time.sleep(3.25)
    require(fault_control(ca,'release',gate)['status']==200,'expired staged transaction barrier failed')
    white.wait_for_function("() => document.documentElement.dataset.onlineAvailability==='unavailable' && !Object.hasOwn(document.documentElement.dataset,'onlineStatus') && document.querySelector('#glcanvas').hidden",timeout=30_000)
    pair.board(1,'White to move');pair.oracle(0)
    results.append({'case':'current_session_expired_during_staged_commit','pass':True,'actual_clock_crossed_original_deadline':True,'durable_rollback_observed':True,'no_ack_or_restored_projection':True})
    pair.close()


def held_delivery(contexts,private,ca,change,results):
    pair=Pair(contexts,private,'delivery-'+change)
    white,black=pair.pages
    target=pair.attachments[1][-1];black_url=black.url
    cookie=next(entry['value'] for entry in contexts[1].cookies() if entry['name']==SESSION_COOKIE)
    black.close()
    control=contexts[1].new_page();control.goto(ORIGIN+GAME_PATH,wait_until='domcontentloaded')
    # Passive recipient has consumed initial attachment frames and has no live
    # browser runtime poll. A real opponent pointer move queues a projection.
    move(white,*MOVES[0],False,pair.match_id);pair.write_audit(1)
    armed=api(control,'/__fixture/publication/arm',{'version':1,'match_id':pair.match_id,'attachment_id':target['attachment_id']},pair.facts[1]['csrf_token'])
    require(armed['status']==200,'actual queued projection delivery barrier failed')
    token=armed['body']['control_token']
    thread,done,observed=start_native_poll(pair.match_id,target['attachment_id'],cookie,pair.facts[1]['csrf_token'])
    try:
        deadline=time.monotonic()+15
        while time.monotonic()<deadline:
            status=publication_control(ca,'status',token)
            require(status['status']==200,'actual publication capture witness unavailable')
            if status['body']['phase']=='held':break
            require(status['body']['phase']=='armed','publication fault had no nonempty real projected frame')
            time.sleep(.025)
        else:raise AcceptanceFailure('actual projected first-frame handoff never reached barrier')
        require(not done.is_set(),'held protected publication finished before authority fault')
        if change=='attachment':
            fresh_page=contexts[1].new_page();fresh_page.on('requestfinished',lambda request:pair.observe_attach_finished(request,1))
            fresh_page.goto(black_url,wait_until='domcontentloaded');pair.board(1,'Black to move',page=fresh_page)
            fresh=pair.attachments[1][-1]
            require(fresh['attachment_id']!=target['attachment_id'] and fresh['operation_scope']==target['operation_scope'],'new current attachment did not fence the held old delivery')
        else:
            changed=api(control,'/__fixture/continuity/authority',{'version':1,'match_id':pair.match_id,'change':change},pair.facts[1]['csrf_token'])
            require(changed['status']==200 and changed['body']['authority_change_committed'] is True,'delivery-time authority or owner change failed')
        if change=='owner_loss':
            # A fresh normal current-authority reattach must claim a new durable
            # owner while the captured old response still retains the old queue.
            fresh=api(white,f'/api/v1/matches/{pair.match_id}/grant',{'version':MATCH_VERSION},pair.facts[0]['csrf_token'])
            require(fresh['status']==200,'new owner current grant failed')
            attached=api(white,f'/api/v1/matches/{pair.match_id}/attach',{'version':MATCH_VERSION,'binding_id':fresh['body']['binding_id']},pair.facts[0]['csrf_token'])
            require(attached['status']==200 and attached['body']['seat']==0 and attached['body']['next_seq']==2,'new fenced owner recovery failed')
        require(publication_control(ca,'release',token)['status']==200,'held protected publication could not release')
        thread.join(timeout=15)
        require(done.is_set() and observed['status']==200 and observed['json_content_type'] and observed['no_store'],'actual native publication response was not observed')
        require(observed['body_bytes']==0 and observed['body_error'],'old authority or owner released protected body bytes')
        require(publication_control(ca,'status',token)['body']['phase']=='suppressed','body cancellation replaced the required actual inner publication error')
        pair.oracle(1,0)
        results.append({'case':change+'_at_held_actual_delivery','pass':True,'nonempty_projected_capture':True,'actual_response_headers':True,'actual_inner_guard_error':True,'native_body_bytes':0,'new_owner_before_old_output':change=='owner_loss'})
    finally:
        if not done.is_set():
            try:publication_control(ca,'release',token)
            except Exception:pass
            thread.join(timeout=15)
        pair.close()


FOCUS_OBSERVER = """() => {
    window.__tabulaFocusFacts={blur:false,focus:false,visibility:0,immediate:null,input:false,awaiting:false};
    addEventListener('blur',event=>{if(event.target===window)window.__tabulaFocusFacts.blur=event.isTrusted;});
    // Registered after the application's synchronous transport focus listener.
    // Only observation/readback occurs here, before another animation frame.
    addEventListener('focus',event=>{
        if(event.target!==window)return;
        const facts=window.__tabulaFocusFacts;
        if(!facts.awaiting || facts.immediate!==null || !event.isTrusted)return;
        facts.focus=true;
        const d=document.documentElement.dataset,c=document.querySelector('#glcanvas');
        const status=document.querySelector('#online-status-container');
        const spans=document.querySelectorAll('#online-status-container span');
        const selection_clear=!!c && !c.hasAttribute('aria-label') && !c.hasAttribute('aria-describedby')
            && (window.getSelection()?.rangeCount ?? 0)===0;
        const concealed=!!c && getComputedStyle(c).visibility==='hidden' && c.getAttribute('aria-hidden')==='true'
            && c.tabIndex===-1 && !!status && status.hidden
            && ['onlineSeat','onlineRevision','onlineStatus'].every(key=>!Object.hasOwn(d,key))
            && Array.from(spans).every(node=>node.textContent.trim()==='');
        let framebuffer='no_live_webgl';
        const g=window.gl;
        if(g && !g.isContextLost()) {
            const w=g.drawingBufferWidth,h=g.drawingBufferHeight;
            if(g.getParameter(g.FRAMEBUFFER_BINDING)!==null)framebuffer='non_default_framebuffer';
            else if(w<=0 || h<=0 || w*h>2097152)framebuffer='invalid_buffer_bounds';
            else {
                const pixels=new Uint8Array(w*h*4);
                g.readPixels(0,0,w,h,g.RGBA,g.UNSIGNED_BYTE,pixels);
                if(g.getError()!==g.NO_ERROR)framebuffer='read_error';
                else {
                    const alpha=g.getContextAttributes()?.alpha===false ? 255 : 0;
                    let clear=true;
                    for(let i=0;i<pixels.length;i+=4)if(pixels[i]!==0||pixels[i+1]!==0||pixels[i+2]!==0||pixels[i+3]!==alpha){clear=false;break;}
                    framebuffer=clear ? 'cleared_pixels' : 'uncleared_pixels';
                }
            }
        }
        facts.immediate={trusted:event.isTrusted,concealed,selection_clear,framebuffer};
    });
    for(const name of ['pointerdown','keydown'])addEventListener(name,event=>{
        const facts=window.__tabulaFocusFacts;
        if(facts.awaiting && facts.focus && event.isTrusted)facts.input=true;
    },{capture:true});
    document.addEventListener('visibilitychange',()=>window.__tabulaFocusFacts.visibility++);
}"""


def focus_only_revoke_game(contexts,private,results):
    pair=Pair(contexts,private,'focus-only-revoke');white,black=pair.pages
    pair.board(0,'White to move')
    bounds=white.locator('#glcanvas').bounding_box()
    require(bounds is not None,'focus-only former board geometry unavailable')
    # A real selection before blur supplies the stale-selection counterexample.
    x,y=board_square(bounds['width'],bounds['height']-56,'f2',False)
    white.mouse.click(bounds['x']+x,bounds['y']+y,delay=70)
    white.wait_for_timeout(150)
    require(not pair.commands[0],'initial selection unexpectedly submitted a move')
    white.evaluate(FOCUS_OBSERVER)
    held_routes=[]
    def hold_revalidation(route):
        require(len(held_routes)<16,'focus-only revalidation exceeded its bounded gate')
        held_routes.append(route)
    # These real requests are held unchanged, never mocked. Blur cancels any
    # earlier in-flight request; all newly generated authority work stays held.
    white.route('**/api/v1/**',hold_revalidation)
    try:
        with white.expect_popup(timeout=15_000) as opened:
            white.evaluate("url => { window.open(url,'_blank','popup=yes,width=500,height=400'); }",ORIGIN+GAME_PATH)
        popup=opened.value;popup.wait_for_load_state('domcontentloaded');popup.bring_to_front()
        white.wait_for_function("() => window.__tabulaFocusFacts.blur===true && document.visibilityState==='visible' && window.__tabulaFocusFacts.visibility===0",timeout=15_000)
        white.wait_for_function(RECOVERING_CONCEALED,timeout=15_000)
        facts=context_facts(popup)
        require(facts['account_id']==pair.facts[0]['account_id'],'focus control popup changed the account')
        logout=api(popup,'/api/v1/auth/logout',{},facts['csrf_token'])
        require(logout['status']==204 and logout['body'] is None,'focus-only current session logout did not commit')
        before=len(pair.commands[0]);white.evaluate("() => { const facts=window.__tabulaFocusFacts; facts.focus=false; facts.immediate=null; facts.input=false; facts.awaiting=true; }")
        popup.close();white.bring_to_front()
        white.wait_for_function("() => window.__tabulaFocusFacts.focus===true && document.visibilityState==='visible' && window.__tabulaFocusFacts.visibility===0",timeout=15_000)
        witness=white.evaluate('window.__tabulaFocusFacts.immediate')
        require(witness is not None and witness['trusted'] is True and witness['concealed'] is True
                and witness['selection_clear'] is True and witness['framebuffer']=='cleared_pixels',
                'native first focus restored pixels, selection or accessibility before current authority')
        # Genuine pointer and keyboard input while current authority is still
        # unresolved, aimed at the former board without requiring visible canvas.
        for square in ('f2','f3'):
            x,y=board_square(bounds['width'],bounds['height']-56,square,False)
            white.mouse.click(bounds['x']+x,bounds['y']+y,delay=70)
        white.keyboard.press('ArrowUp');white.keyboard.press('Enter')
        require(white.evaluate('window.__tabulaFocusFacts.input') is True,
                'focus-only blocked input did not exercise a trusted native gesture')
        require(len(pair.commands[0])==before,'uncertain focus restoration submitted a board command')
    finally:
        white.unroute('**/api/v1/**',hold_revalidation)
        for route in held_routes:
            try:route.continue_()
            except Exception:pass # Canceled earlier generations cannot be revived.
    white.wait_for_function("() => document.documentElement.dataset.onlineAvailability==='unavailable' && !Object.hasOwn(document.documentElement.dataset,'onlineStatus') && document.querySelector('#glcanvas').hidden && document.querySelector('#glcanvas').width===0 && document.querySelector('#glcanvas').height===0",timeout=30_000)
    require(len(pair.commands[0])==before,'focus restoration sent an input before current authority')
    denied(api(white,f'/api/v1/matches/{pair.match_id}/poll',
               {'version':MATCH_VERSION,'attachment_id':pair.attachments[0][-1]['attachment_id']},pair.facts[0]['csrf_token']),
           {401},'revoked focus-restored browser received protected output')
    pair.oracle(0);pair.write_audit(0)
    results.append({'case':'trusted_focus_only_restore_revalidates_revoked_authority','pass':True,
                    'actual_native_popup':True,'trusted_window_blur_and_focus':True,'visibility_signals_absent':True,
                    'immediate_focus_surface_concealed':True,'cleared_framebuffer_before_restored_paint':True,
                    'genuine_blocked_input_attempt':True,'actual_protected_poll_401':True,
                    'no_restored_projection_or_input':True})
    pair.close()


def run(args):
    require(os.environ.get('CI') in ('true','1') and os.environ.get('TABULA_ONLINE_MATCH_DISPOSABLE')=='1','explicit CI-only continuity acceptance opt-in is required')
    private,artifacts,ca=Path(args.private),Path(args.artifacts),Path(args.ca)
    supervisor=SupervisorClient(private/'process-supervisor.sock')
    results={'version':1,'optional':{},'status':'fail','actual_chromium_processes':2,'real_postgres':True,'normal_tls_verification':True,'cases':[]}
    contexts=[]
    try:
        with sync_playwright() as playwright:
            for role in ('continuity-white','continuity-black'):
                home,profile=setup_browser_trust(private,role,ca);environment=os.environ.copy();environment['HOME']=str(home);environment['XDG_DATA_HOME']=str(home/'.local/share')
                contexts.append(playwright.chromium.launch_persistent_context(str(profile),headless=True,channel='chromium',chromium_sandbox=True,env=environment,viewport={'width':1100,'height':850},reduced_motion='reduce',locale='en-US'))
                install_actual_response_observer(contexts[-1])
            pids=[]
            for context in contexts:
                inspector=context.browser.new_browser_cdp_session()
                try:pids.extend(process['id'] for process in inspector.send('SystemInfo.getProcessInfo')['processInfo'] if process['type']=='browser')
                finally:inspector.detach()
            require(len(pids)==2 and len(set(pids))==2,'continuity opponents share an actual browser process')
            results['distinct_browser_processes_verified']=True
            # Reuse exact clean-build provenance without retaining private source input.
            evidence_dir=artifacts/'continuity-ui';evidence_dir.mkdir(exist_ok=True)
            for name in ('source-sha.txt','source-tree.txt'):(evidence_dir/name).write_bytes((artifacts/name).read_bytes())
            evidence=CaptureEvidence(evidence_dir,Path(__file__).resolve().parents[2])
            evidence.build['fixture_build']='continuity-test; native debug; debug info disabled';evidence.write()
            network_refresh_game(contexts,private,ca,results['cases'],evidence)
            crash_game(contexts,private,ca,supervisor,False,results['cases'])
            crash_game(contexts,private,ca,supervisor,True,results['cases'])
            for point in ('before_commit','after_commit'):apply_and_committed_refresh_game(contexts,private,ca,point,results['cases'])
            same_record_rotation_game(contexts,private,ca,results['cases'])
            restart_between_grant_and_attach_game(contexts,private,ca,supervisor,results['cases'])
            changed_identity_game(contexts,private,ca,True,results['cases'])
            changed_identity_game(contexts,private,ca,False,results['cases'])
            repeated_recovery_and_navigation(contexts,private,results['cases'],results['optional'])
            for mode in ('evicted','expired'):retired_receipt_game(contexts,private,ca,supervisor,mode,results['cases'])
            supervisor.kill();supervisor.restart('normal')
            ready=contexts[0].new_page();ready_server(ready);ready.close()
            authority_expiry_during_commit(contexts,private,ca,results['cases'])
            for change in ('revoke','expire','epoch','membership'):authority_before_commit(contexts,private,ca,supervisor,change,results['cases'])
            for change in ('revoke','expire','epoch','membership','owner_loss','attachment'):held_delivery(contexts,private,ca,change,results['cases'])
            # Window-focus evidence needs actual headed native windows. The CI
            # wrapper supplies its disposable Xvfb display; absent support fails.
            focus_contexts=[]
            try:
                for role in ('focus-white','focus-black'):
                    home,profile=setup_browser_trust(private,role,ca);environment=os.environ.copy();environment['HOME']=str(home);environment['XDG_DATA_HOME']=str(home/'.local/share')
                    focus_contexts.append(playwright.chromium.launch_persistent_context(str(profile),headless=False,channel='chromium',chromium_sandbox=True,env=environment,viewport={'width':1100,'height':850},reduced_motion='reduce',locale='en-US'))
                    install_actual_response_observer(focus_contexts[-1])
                focus_pids=[]
                for context in focus_contexts:
                    inspector=context.browser.new_browser_cdp_session()
                    try:focus_pids.extend(process['id'] for process in inspector.send('SystemInfo.getProcessInfo')['processInfo'] if process['type']=='browser')
                    finally:inspector.detach()
                require(len(focus_pids)==2 and len(set(focus_pids))==2,'focus-only opponents share an actual native process')
                focus_only_revoke_game(focus_contexts,private,results['cases'])
            finally:
                for context in reversed(focus_contexts):context.close()
            require(len(results['cases'])==28,'continuity acceptance selection was incomplete')
            results['status']='pass'
    except Exception as error:
        results['failure_class']=exception_class(error)
        raise
    finally:
        for context in reversed(contexts):
            try:context.close()
            except Exception:pass
        (artifacts/'continuity-result.json').write_text(json.dumps(results,indent=2)+'\n')


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--private',required=True);parser.add_argument('--artifacts',required=True);parser.add_argument('--ca',required=True);args=parser.parse_args()
    try:run(args)
    except AcceptanceFailure as error:print('FAIL: '+str(error));return 1
    except Exception:print('FAIL: actual continuity infrastructure or browser interaction failed');return 1
    print('PASS: actual rendered continuity fault partitions');return 0
if __name__=='__main__':raise SystemExit(main())
