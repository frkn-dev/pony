# Node pool / pool_translation

Локализуемое имя группы (пула) серверов. Позволяет показывать в клиенте
переведённое имя пула, не меняя `connection_label` (совместимость со старыми
клиентами).

## Конфиг ноды

`src/bin/node/node-example.toml`, секция `[node]`:

```toml
# pool = "Finland"
# pool_translation = { en = "Finland", ru = "Финляндия", uk = "Фінляндія" }
```

- `pool` — необязательное имя группы (например, `Finland`). Пустая строка =
  отсутствует.
- `pool_translation` — необязательная карта `код языка -> отображаемое имя`.
  Ключи trim/lowercase; `ua` принимается как алиас `uk`; пустые ключи/значения
  отбрасываются. Пустая карта → `None`.

Оба поля нормализуются в `fcore::config::settings`
(`normalize_pool` / `normalize_pool_translation`), затем попадают в `Node` и
регистрируются на API (`NodeRequest`).

## API

`POST /node` / `POST /private/nodes` принимают необязательные поля `pool` и
`pool_translation` (`#[serde(default)]` — старые ноды их не шлют). На границе
API `pool_translation` нормализуется ещё раз (алиас `ua`→`uk`).

`GET /v1/services` (`handlers/amnezia.rs`) отдаёт их в каждом
`connections[]` и `available_countries[]`:

```json
{
  "connection_label": "Finland · AmneziaWG",
  "pool": "Finland",
  "pool_translation": { "en": "Finland", "ru": "Финляндия", "uk": "Фінляндія" }
}
```

- `connection_label` **не меняется** — старые клиенты видят прежний ответ.
- `pool` / `pool_translation` сериализуются только когда заданы
  (`skip_serializing_if = "Option::is_none"`), поэтому для старых нод форма
  ответа прежняя.

## Хранение

`dev/dev.sql`:

```sql
ALTER TABLE nodes ADD COLUMN IF NOT EXISTS pool TEXT;
ALTER TABLE nodes ADD COLUMN IF NOT EXISTS pool_translation JSONB;
```

`PgNode::upsert` пишет оба поля, но при конфликте использует
`COALESCE(EXCLUDED.pool, nodes.pool)` /
`COALESCE(EXCLUDED.pool_translation, nodes.pool_translation)`: пустое значение
при повторной регистрации **не затирает** уже сохранённые переводы. `PgNode::all`
читает их обратно в `Node`.
