"""Run the existing strict raw shell budget; no cap override or waiver."""
from __future__ import annotations
import importlib.util
import json
from pathlib import Path

root=Path(__file__).resolve().parents[2]
path=root/'tools/tests/check-loading-budgets.py'
spec=importlib.util.spec_from_file_location('existing_loading_budget_owner',path)
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
out=root/'verification/current-ui-report-artifacts'
out.mkdir(parents=True,exist_ok=True)
receipt={'owner':'tools/tests/check-loading-budgets.py::shell_budget',
         'unchanged_limit_bytes':module.SHELL_RAW_LIMIT,
         'actual_shell_wasm':[{'path':p.name,'bytes':p.stat().st_size} for p in sorted((root/'apps/web/dist').glob('*.wasm'))]}
try:
    receipt.update(status='PASS',result=module.shell_budget(root/'apps/web/dist'))
except AssertionError as error:
    receipt.update(status='FAIL',failure=str(error))
    (out/'shell-budget.json').write_text(json.dumps(receipt,indent=2)+'\n')
    raise
(out/'shell-budget.json').write_text(json.dumps(receipt,indent=2)+'\n')
