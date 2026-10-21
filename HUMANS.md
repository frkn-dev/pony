# HUMANS.md

## Current
- **Repo:** ffcore
- **Branch:** `feature/share-node-pick` (локально 0.6.62, shared subscriptions), не запушено
- **Task:** «поделиться всем env» — ссылка-подписка `frkn://sub/shared/<uuid>`
- **Status:** код готов и собран, тесты 99+96 зелёные; на прод не выкачено
- **Updated:** 2026-10-21

## Done
- 2026-10-21 — проверка вторым прогоном: шарить personal/selfhosted env теперь можно и без живого коннекшена — `owns_env = scope_env == env || есть коннекшен в env` (env принадлежит подписке по построению). Общие env по-прежнему требуют коннекшен. `cargo test --lib` 99, `cargo test --bin api` 96 — зелёные.
- 2026-10-21 — 0.6.62 (локально): shared subscriptions. `share_tokens.kind` (`conn`/`sub`), `source_connection_id` nullable; `POST /share/sub` (mgmt) `{subscription_id, env, proto}` → `frkn://sub/shared/<uuid>`, child-коннекшен `issued_via='shared_sub'` без метки, мят идемпотентен по `(subscription, "env · proto")`, шарить можно только env, который реально есть у подписки; `/v1/services` с `sub`-токеном отдаёт все серверы env+proto; `/v1/config` с `sub`-токеном уважает payload `node_id` в рамках env; в `GET /subscription/{id}` добавлен `shared_subs[]`.
- 2026-10-21 — AGENTS.md переписан: карта кода/доков, build-флаги (api/auth без default-features, node с xray/wireguard/amnezia-wg), миграции только на деплое, релиз через server-release.sh, грабли; не закоммичено.
- 2026-10-21 — v0.6.60: env-wide share-токены (`share_tokens.node_id` nullable, `/sub/<token>` отдаёт все онлайн-ноды env, `/v1/config` для env-токена = конфиг выбранной ноды + `node_ips` пула).
- 2026-10-21 — v0.6.59: `GET /private/nodes/stats?subscription_id=` (живые CPU/RAM/диск/сеть + inbound трафик/коннекты) и `GET /private/nodes/{uuid}/series` (даунсемпл ~240 бакетов, 24ч/7д), владение проверяется по personal env.
- 2026-10-21 — имена серверов в `/v1/services`: `Устройство (NL4)`, для пулов `Устройство (Finland)`; безымянные коннекшены — лейбл ноды, как раньше. ` · ` остаётся под суффикс протокола.
- 2026-10-21 — тег `v0.6.59`, GitHub Release, api на `141.133.173.16` (`Ok (v0.6.59)`), бэкап `api.bak-0.6.58-*`.
- 2026-10-21 — почищены ветки: 19 смердженных + 5 старых (они есть на origin).
- 2026-10-20 — v0.6.58: catalog-pool-version + healthcheck-version, api задеплоен.
- 2026-10-20 — v0.6.57: Clash + Hysteria2, собран на us.frkn.org.

## Next
- [ ] решить, персистить ли метрики (сейчас рестарт api обнуляет историю графиков).
- [ ] патч `src/bin/node/tasks.rs` (`SUB: Failed to handle messages`) — применять или нет.
- [ ] выкатить 0.6.62 api + mrkting 0.2.39 (после «мержим»), проверить шар одной ноды и шара региона на `dev`.
- [ ] dopamine: сборка с `frkn://sub/shared/` (локально собирается, не закоммичено).

## Blockers / Notes
- Один конфиг = один сервер: у каждой ноды свои ключи (AWG pubkey / Reality), поэтому env-wide «одна карточка с фейловером» невозможна вне пула.
- `/subscription/{id}` (mrkting) отдаёт `connections[].id`, но `node_id` там нет — для тестов брать id оттуда.
- SSH на прод: ключ `~/.ssh/ed25519_frkn`; GitHub — `~/.ssh/id_rsa_gh_2pizza` (после рестарта агента добавлять `ssh-add`.
- Мерджи только `--no-ff`, версия бампается на ветке.
- Секреты не коммитить.

## Related
- `frkn.org/HUMANS.md`, `frkn-services/HUMANS.md` (Task DEVICES-ENV)
