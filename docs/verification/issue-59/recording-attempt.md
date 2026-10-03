# Optional video export attempt

This is a manually recorded attempt receipt from the supported browser tool
observations, not a recovered console export or a controlled measurement row.

At `http://127.0.0.1:8059/script-light/`, the observation host's Record button
was clicked. After twenty seconds the accessibility tree exposed the completed
`Download canvas recording` link with a local Blob URL. The runtime console
reported source `issue59-source-60460d1affc8c682`, 600 samples, 20 completed script
steps, 44 accepted inputs, 25 board cells and checkpoint
`b7e81e41d5bd48f076a736858b6855b663591034127628be52b887ae1ce19a73`.
The record-control click counted as one uncontrolled input event. That run is
excluded from the performance table.

The supported link `downloadMedia` call stalled for thirty seconds and reset
the automation session. Rebinding the same tab showed the completed link still
present. A normal click with a ten-second download-event wait also timed out.
No exported video file was confirmed. Video delivery is **BLOCKED**; there was
no browser permission/approval bypass and no further export workaround. Console
capture history was lost on reconnection, so no empty log is offered as evidence.

The before/after board and Sprite/theme screenshots are the delivered runtime
visual artifacts. The isolated host retains optional recording for manual
reproduction on a browser with working download support.
