# Fc0re — AGENTS.md

Cluster control plane for Xray/Shadowsocks/Hysteria2/Wireguard/Amnezia-Wireguard/MTproto.
Binaries: `api`, `node`, `auth`. See `README.md` for architecture and features.

## HUMANS.md — work status

The repo root has `HUMANS.md`: a human-readable status file with the current branch, task, done items, next steps, and blockers.

- At task start, update `Current` (branch, task, status, date).
- At task end, add a `Done` entry (date, short description, commit hash if any) and refresh `Next`.
- Keep `Done` short: last 5–10 items; full history lives in git.
- `HUMANS.md` is "what we are doing now"; `AGENTS.md` is "how we work". Do not duplicate rules.
- Never put secrets in it: tokens, passwords, private keys.
- For cross-repo work, use the same short Task ID and link the other `HUMANS.md` files.
- If blocked, record it under `Blockers / Notes`, not only in chat.

## Git

- Commit and push only on explicit request from the user.
- Do not commit build artifacts, local snapshots, or secrets.
- Keep changes minimal and consistent with the surrounding code.
