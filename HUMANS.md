# HUMANS.md

## Current
- **Repo:** ffcore
- **Branch:** `main` (смерджено, запушено; release `v0.6.62`)
- **Task:** «поделиться всем env» — ссылка-подписка `frkn://sub/shared/<uuid>`
- **Status:** api **0.6.62 на проде** с 2026-10-22 00:31 (`Ok (v0.6.62)`, ноды вернулись Online, миграций нет); осталось проверить шару одной ноды и региона живьём
- **Updated:** 2026-10-22

## Done
- 2026-10-22 — **заблокирована lite-подписка** `d6a469c4…` (1 ГиБ, 11 коннектов dev/ru) через `DELETE /admin/api/subscriptions/{id}`: `is_deleted=t` в БД, все 11 коннектов `is_deleted=t`/`deleted_reason=manual`, в памяти api `is_active=false`, на `cz2` пиров по её AWG-адресам нет. Отдельного флага «block» в ffcore нет: у lite (`expires_at NULL`) единственный рычаг — `is_deleted`; обнуление `limit_bytes` не блокирует (реконсайлер пересоздаёт коннекты, т.к. `is_active()` для lite = true). Прямой SQL не виден api до `db_sync_interval_sec` (1000 с), через admin API — сразу. mrkting при удалении не уведомляется — платёжная запись/ЛК остаются.
- 2026-10-22 — **проверены добавление/удаление коннектов на живой ноде** (`cz2.frkn.org`, personal env подписки `c6831dad…`, api 0.6.62): `POST /connection` (AmneziaWg) → новый пир на `awg0` за ~5с, `allowed-ips` = выданному адресу; `DELETE /connection` → пир снят (2 → 1 пир, держится 60с), строка `is_deleted=t`, `deleted_reason=manual`. Hysteria2: `/auth` с токеном созданного коннекта → `{"ok":true,"id":…}`, после DELETE → `{"ok":false}` — отзыв работает. Сверка ноды с БД сошлась точно: 4 активных коннекта env ↔ 4 пира на `awg0`+`awg1` по allowed-ips, сирот нет. Найдено два дефекта: (1) повторный `POST /connection` на ту же пару `(subscription, env, proto)` отдаёт **500** с сырым текстом ошибки БД (`connections_single_proto_idx`) вместо 409; (2) env с лишним префиксом (`custom` + `custompersonal…`) принимается молча — коннект создаётся в env, которого нет ни на одной ноде, и hysteria-токен при этом выдаётся.
- 2026-10-22 — **api 0.6.62 выкачен на прод** (`141.133.173.16`, `/opt/api/api`, systemd `api`): релиз опубликован из готового билда на билд-сервере (`gh release create` не доехал 21.10 после сборки в 23:15, пересборки не было), бэкап `api.bak-0.6.60`, `healthcheck` → `Ok (v0.6.62)`. Миграций между 0.6.60 и 0.6.62 нет. 15 нод ушли в Offline на рестарте и все вернулись Online; метрики нод обнулились (in-memory).
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
- [ ] ветка `feature/share-cleanup` (не закоммичена, тесты не гонялись): миграция `0004_conn_invariants.sql` (исключить `issued_via='shared_sub'` из `connections_single_proto_idx` + `connections_token_uniq`), идемпотентный `POST /connection` (200 + существующий id вместо 500/409), `Env::Deserialize` через одну нормализацию префикса `custom`, env-гейт `validate_env_served` в `create_connection_inner`. `cargo check` для `--bin api` и `--bin auth` зелёный; `cargo test` и деплой — по отдельному согласию.
- [ ] решить по `share_feed_url`: сейчас `base_url = https://frkn.org` даёт `https://frkn.org/sub/<token>`, а nginx отдаёт там HTML кабинета; сырой фид живёт на `api.frkn.org/sub/<token>`.
- [ ] решить, персистить ли метрики (сейчас рестарт api обнуляет историю графиков).
- [ ] патч `src/bin/node/tasks.rs` (`SUB: Failed to handle messages`) — применять или нет.
- [ ] проверить живьём шару одной ноды и шару региона на `dev` (api 0.6.62 + mrkting 0.2.42 уже на проде).
- [ ] dopamine: сборка с `frkn://sub/shared/` (локально собирается, не закоммичено).

## Blockers / Notes
- Один конфиг = один сервер: у каждой ноды свои ключи (AWG pubkey / Reality), поэтому env-wide «одна карточка с фейловером» невозможна вне пула.
- `/subscription/{id}` (mrkting) отдаёт `connections[].id`, но `node_id` там нет — для тестов брать id оттуда.
- SSH на прод: ключ `~/.ssh/ed25519_frkn`; GitHub — `~/.ssh/id_rsa_gh_2pizza` (после рестарта агента добавлять `ssh-add`.
- Мерджи только `--no-ff`, версия бампается на ветке.
- Секреты не коммитить.

## Related
- `frkn.org/HUMANS.md`, `frkn-services/HUMANS.md` (Task DEVICES-ENV)
