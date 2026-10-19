# Статистика «Помочь улучшить Jiyda» — сервер приёма

Jiyda отправляет статистику, только если пользователь сам включил «Помочь улучшить Jiyda»
(Настройки → Конфиденциальность и защита или знакомство при первом запуске). По умолчанию выключено.
Что именно уходит — `src/telemetry.rs` и кнопка «Что отправляется» в Настройках.

Пока адрес приёма не задан, в сборках Jiyda этой функции нет совсем: её не видно в
Настройках, ничего не считается и не отправляется.

## Что здесь

- `worker.js` — Cloudflare Worker: принимает `POST /v1/report`, строго проверяет отчёт и
  пишет его в D1. IP-адреса и заголовки не сохраняет.
- `schema.sql` — таблица `reports`: одна строка — один день одной установки.
- `wrangler.toml` — настройки Worker (журнал запросов Cloudflare выключен).
- `metrics.sql` — DAU, MAU, удержание D1/D7/D30, сессии без сбоев, обновления, активация.

## Как развернуть (один раз, ~10 минут)

Нужен бесплатный аккаунт Cloudflare и Node.js.

```sh
npm install -g wrangler
wrangler login
cd telemetry
wrangler d1 create jiyda-telemetry          # скопируйте database_id в wrangler.toml
wrangler d1 execute jiyda-telemetry --remote --file schema.sql
wrangler deploy                              # выведет адрес: https://jiyda-telemetry.<имя>.workers.dev
```

Проверка (должно вернуть 204):

```sh
curl -i -X POST https://jiyda-telemetry.<имя>.workers.dev/v1/report \
  -H "Content-Type: application/json" \
  -d '{"id":"0123456789abcdef0123456789abcdef","day":"2026-10-19","version":"0.6.5","windows":"10.0.26100","counts":{"app_started":1},"tabs":"2-5","default_browser":false}'
```

Затем удалите проверочную строку:
`wrangler d1 execute jiyda-telemetry --remote --command "DELETE FROM reports WHERE id='0123456789abcdef0123456789abcdef'"`.

## Включить в сборках Jiyda

GitHub → репозиторий → Settings → Secrets and variables → Actions → **Variables** →
New repository variable:

- Name: `TELEMETRY_URL`
- Value: `https://jiyda-telemetry.<имя>.workers.dev/v1/report`

Следующая сборка в CI (и следующий релиз) получит этот адрес. Адрес должен начинаться с
`https://` — иначе Jiyda его не примет.

**До публикации такой версии** обновите Privacy (site/*/privacy.html) и строку «Jiyda не
собирает… статистику использования» на лендинге — тексты подготовлены в ветке
`site-telemetry`.

## Хранение

Отчёты старше 13 месяцев не нужны для метрик — их можно удалять раз в месяц:

```sh
wrangler d1 execute jiyda-telemetry --remote --command "DELETE FROM reports WHERE day < date('now', '-13 months')"
```

Пользователь, выключивший статистику, получает новый случайный номер, если включит её
снова, — старые строки с ним больше не связаны.
