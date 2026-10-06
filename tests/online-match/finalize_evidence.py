"""Closed receipts require every actual browser and independent durable oracle."""
import argparse
import json
from pathlib import Path

PASS_MESSAGE = "PASS: independent actual Chromium opponents completed rendered Chess and all mandatory continuity fault partitions through real durable authority\n"
CONTINUITY_CASES = frozenset({
    'restart_between_grant_and_attach_recovers_unchanged_old_csrf',
    'old_signed_grant_with_fresh_current_csrf_requires_fresh_grant',
    'same_auth_record_rotation_preserves_uncertain_original_operation', 'trusted_focus_only_restore_revalidates_revoked_authority', 'drop_before_send_refresh_pending', 'drop_during_staged_apply_commit',
    'drop_after_pure_apply_before_append','refresh_committed_before_original_ack',
    'new_auth_record_never_replays_old_operation','cross_account_never_restores_old_projection_or_replays',
    'repeated_recovery_explicit_retry','cancel_leave_cancel_back_then_fresh_history_return',
    'current_session_expired_during_staged_commit', 'lost_output_poll_refresh_committed', 'uncommitted-crash', 'committed-crash',
    'evicted_original_receipt_unknown_full_resync', 'expired_original_receipt_unknown_full_resync',
    *(change+'_ordered_before_command_commit' for change in ('revoke','expire','epoch','membership')),
    *(change+'_at_held_actual_delivery' for change in ('revoke','expire','epoch','membership','owner_loss','attachment')),
})
AUDIT_PREFIXES = {
    'restart-grant-attach':4, 'same-record-rotation':4, 'focus-only-revoke':0, 'pure-apply-loss':4,'committed-refresh':4,'new-record-scope':1,'cross-account-scope':1,'repeat-navigation':4,
    'expiry-during-commit':0, 'network-refresh':4, 'uncommitted-crash':4, 'committed-crash':4,
    'evicted-receipt':4, 'expired-receipt':4,
    **{'authority-'+change:0 for change in ('revoke','expire','epoch','membership')},
    **{'delivery-'+change:1 for change in ('revoke','expire','epoch','membership','owner_loss','attachment')},
}


def actual_continuity_receipts(artifacts: Path) -> bool:
    """Missing/empty/duplicate/incomplete selections cannot seal a PASS."""
    try:
        raw=(artifacts/'continuity-result.json').read_bytes()
        if len(raw)>131072:return False
        result=json.loads(raw)
        cases=result['cases']
        if (result.get('status')!='pass' or result.get('distinct_browser_processes_verified') is not True
                or not isinstance(cases,list) or len(cases)!=len(CONTINUITY_CASES)
                or {case.get('case') for case in cases}!=CONTINUITY_CASES
                or any(case.get('pass') is not True for case in cases)):
            return False
        for case in cases:
            if case['case']=='restart_between_grant_and_attach_recovers_unchanged_old_csrf':
                if any(case.get(key) is not True for key in ('actual_sigkill_reaped','unchanged_request_403_without_frames','fresh_context_grant_scope_before_restore','exact_original_ack')):return False
            if case['case']=='old_signed_grant_with_fresh_current_csrf_requires_fresh_grant':
                if any(case.get(key) is not True for key in ('separate_current_csrf_probe','exact_409_fresh_grant_required','no_attachment_projection_or_apply_from_old_grant')):return False
            if case['case']=='same_auth_record_rotation_preserves_uncertain_original_operation':
                if any(case.get(key) is not True for key in ('credential_rotated','reattach_required_observed','same_operation_scope','exact_original_ack')):return False
            if case['case'].endswith('_at_held_actual_delivery'):
                if (case.get('native_body_bytes')!=0 or case.get('nonempty_projected_capture') is not True
                        or case.get('actual_inner_guard_error') is not True):return False
            if case['case'] in ('committed-crash','uncommitted-crash'):
                if case.get('actual_sigkill_reaped') is not True or case.get('fresh_fenced_owner') is not True:return False
        files=list((artifacts/'continuity-audits').glob('*.json'))
        if len(files)!=len(AUDIT_PREFIXES):return False
        for label,expected in AUDIT_PREFIXES.items():
            raw=(artifacts/'continuity-audits'/('continuity-audit-'+label+'.json')).read_bytes()
            if len(raw)>8192:return False
            audit=json.loads(raw)
            if (audit.get('status')!='pass' or audit.get('actual_postgres') is not True
                    or audit.get('exact_runtime_recovery') is not True
                    or audit.get('accepted_inputs')!=expected or audit.get('audit_client_output') is not False):return False
        return True
    except (OSError,ValueError,KeyError,TypeError,AttributeError):return False


def finalize(artifacts: Path, browser_status: int, audit_status: int,
             audit_input_present: bool, continuity_status: int=1,
             continuity_audit_status: int=1, continuity_audit_count: int=0) -> int:
    if (any(isinstance(value,bool) or not isinstance(value,int) or not 0<=value<=255
            for value in (browser_status,audit_status,continuity_status,continuity_audit_status))
            or not isinstance(audit_input_present,bool) or isinstance(continuity_audit_count,bool)
            or not isinstance(continuity_audit_count,int) or not 0<=continuity_audit_count<=128):
        raise ValueError("invalid closed execution status")
    continuity_pass=(continuity_status==0 and continuity_audit_status==0
                     and continuity_audit_count==len(AUDIT_PREFIXES) and actual_continuity_receipts(artifacts))
    passed=browser_status==0 and audit_status==0 and audit_input_present and continuity_pass
    receipt={'version':2,'browser_exit_status':browser_status,
             'durable_audit':'pass' if audit_input_present and audit_status==0 else 'fail' if audit_input_present else 'not_run',
             'main_terminal_confirmed':audit_input_present,'continuity_exit_status':continuity_status,
             'continuity_independent_audits':continuity_audit_count,
             'all_continuity_checks_passed':continuity_pass,'all_mandatory_checks_passed':passed}
    (artifacts/'audit-result.json').write_text(json.dumps(receipt,indent=2)+'\n')
    result=artifacts/'result.txt'
    if passed:result.write_text(PASS_MESSAGE)
    else:result.unlink(missing_ok=True)
    return 0 if passed else 1


def main() -> int:
    parser=argparse.ArgumentParser()
    parser.add_argument('--artifacts',required=True,type=Path)
    parser.add_argument('--browser-status',required=True,type=int)
    parser.add_argument('--audit-status',required=True,type=int)
    parser.add_argument('--audit-input-present',required=True,type=int,choices=(0,1))
    parser.add_argument('--continuity-status',required=True,type=int)
    parser.add_argument('--continuity-audit-status',required=True,type=int)
    parser.add_argument('--continuity-audit-count',required=True,type=int)
    args=parser.parse_args()
    return finalize(args.artifacts,args.browser_status,args.audit_status,args.audit_input_present==1,
                    args.continuity_status,args.continuity_audit_status,args.continuity_audit_count)
if __name__=='__main__':raise SystemExit(main())
