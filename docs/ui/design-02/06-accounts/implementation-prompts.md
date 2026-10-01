## Prompt PR A — spec accounts/social
```text
Pin develop HEAD loveoverflowcom/tabula; đọc AGENTS.md / doc 00 / doc 03 / doc 04 / doc 05 / doc 07, apps/web và storage/lobby scaffolds. Dùng pack 06-accounts viết spec cho màn 15, 16, 20, 21 trong docs/ui/screens/ với form/route/state matrix, typed-data owner, permission+privacy, local escape path và foundation mapping. Mark auth/social APIs proposed until implemented. Không tạo auth provider/token persistence mới, không suy quyền từ mock DOM hoặc stats mẫu, không vượt gate Phase 5. Chốt error copy không account enumeration và screen reader field/error/focus behavior.
```

## Prompt PR B — auth/profile slice sau gate
```text
Khi Phase 4 identity/session APIs và Phase 5 shell gate được chứng minh, implement login/register và self-profile read-only trước. Pin HEAD, read AGENTS.md / doc 00 / doc 03 / doc 04/spec 06, dùng protocol/session adapter thực cùng foundation. Không tự chọn OAuth provider hoặc lưu password/token khác policy; no localStorage mock-auth. Preserve local play/back and safe redirects. Test valid/invalid/pending/expired-session, duplicate submit, field errors/focus/IME/longtext and access permissions. Sau đó edit-profile chỉ theo approved API/fields, một PR riêng nếu mutation lớn. Run targeted security/UI checks+just check.
```

## Prompt PR C — friends/presence sau contract
```text
Chỉ khi lobby/social typed APIs thật có, implement friends list/presence/invites với server permissions và stale/unknown status. Read AGENTS.md / doc 00 / doc 03 / doc 04 and spec 06; reuse shell single lobby socket, không thêm match socket. Invite idempotency/pending/expiry và unauthorized failures rõ; UI không tự accept/create friendship hoặc send real invite chỉ vì mock state toggled. Test duplicate requests, late presence update, expired invite, viewer permission and navigation. Không triển khai matchmaking/voice; report actual integration vs doubles.
```

