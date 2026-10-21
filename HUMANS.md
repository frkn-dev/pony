# HUMANS.md

## Current
- **Repo:** ffcore
- **Branch:** `main` (синхронно с `origin/main`, тег `v0.6.59`)
- **Task:** DEVICES-ENV — env-wide устройства + статистика своих нод для ЛК
- **Status:** v0.6.59 собрана и задеплоена на прод-api
- **Updated:** 2026-10-21

## Done
- 2026-10-21 — v0.6.59: `GET /private/nodes/stats?subscription_id=` (живые CPU/RAM/диск/сеть + inbound трафик/коннекты) и `GET /private/nodes/{uuid}/series` (даунсемпл ~240 бакетов, 24ч/7д), владение проверяется по personal env.
- 2026-10-21 — имена серверов в `/v1/services`: `Устройство (NL4)`, для пулов `Устройство (Finland)`; безымянные коннекшены — лейбл ноды, как раньше. ` · ` остаётся под суффикс протокола.
- 2026-10-21 — тег `v0.6.59`, GitHub Release, api на `141.133.173.16` (`Ok (v0.6.59)`), бэкап `api.bak-0.6.58-*`.
- 2026-10-21 — почищены ветки: 19 смердженных + 5 старых (они есть на origin).
- 2026-10-20 — v0.6.58: catalog-pool-version + healthcheck-version, api задеплоен.
- 2026-10-20 — v0.6.57: Clash + Hysteria2, собран на us.frkn.org.

## Next
- [ ] env-wide share-токен: `share_tokens.node_id` → nullable, уникальный индекс `(source_connection_id, label) WHERE node_id IS NULL`; минт без пина для unpinned-коннекшенов; `/sub/<token>` отдаёт все онлайн-ноды env.
- [ ] `/v1/config` с share-токеном: для env-токена отдавать конфиг одной ноды + `node_ips` пула (фейловер только внутри пула, ключи per-node).
- [ ] решить, персистить ли метрики (сейчас рестарт api обнуляет историю графиков).
- [ ] патч `src/bin/node/tasks.rs` (`SUB: Failed to handle messages`) — применять или нет.

## Blockers / Notes
- Один конфиг = один сервер: у каждой ноды свои ключи (AWG pubkey / Reality), поэтому env-wide «одна карточка с фейловером» невозможна вне пула.
- Мерджи только `--no-ff`, версия бампается на ветке.
- Секреты не коммитить.

## Related
- `frkn.org/HUMANS.md`, `frkn-services/HUMANS.md` (Task DEVICES-ENV)
