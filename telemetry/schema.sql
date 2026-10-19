-- База D1 для статистики «Помочь улучшить Jiyda» (worker.js). Одна строка — один день
-- одной установки. IP-адресов, адресов страниц и любого текста здесь нет.
CREATE TABLE IF NOT EXISTS reports (
  id TEXT NOT NULL,              -- случайный номер установки (стирается, когда статистику выключают)
  day TEXT NOT NULL,             -- ГГГГ-ММ-ДД, местное время пользователя
  version TEXT NOT NULL,         -- версия Jiyda
  windows TEXT NOT NULL,         -- 10.0.<сборка>
  tabs TEXT NOT NULL,            -- открытые страницы диапазоном: 1, 2-5, 6-10, 11-20, 21-50, 51+
  default_browser INTEGER NOT NULL,
  app_started INTEGER NOT NULL,
  app_crashed INTEGER NOT NULL,
  update_success INTEGER NOT NULL,
  update_failed INTEGER NOT NULL,
  workspace_used INTEGER NOT NULL,
  command_palette_used INTEGER NOT NULL,
  import_completed INTEGER NOT NULL,
  onboarding_completed INTEGER NOT NULL,
  PRIMARY KEY (id, day)
);
CREATE INDEX IF NOT EXISTS reports_day ON reports (day);
