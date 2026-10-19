//! «Помочь улучшить Jiyda»: счётчики событий и отправка отчёта раз в сутки
//! (что именно уходит — src/telemetry.rs).

use super::*;
use crate::telemetry::{self, Event, Snapshot};

impl App {
    /// Учесть событие (статистика выключена — ничего не происходит).
    pub(super) fn metric(&mut self, ev: Event) {
        self.telemetry.record(ev, &shield::today());
    }

    fn telemetry_snapshot(&self) -> Snapshot {
        let pages = (0..self.spaces.len()).map(|i| self.space_pages(i).iter().filter(|t| t.is_now()).count()).sum();
        Snapshot {
            version: update::CURRENT.to_string(),
            windows: telemetry::windows_version(),
            pages,
            default_browser: instance::is_default_browser(),
        }
    }

    /// Из `tick`: пора — отправить отчёты за прошедшие дни (в фоне).
    pub(super) fn telemetry_tick(&mut self, t: Instant) {
        if !self.telemetry.is_on() || self.telemetry_busy || t < self.next_telemetry {
            return;
        }
        self.next_telemetry = t + TELEMETRY_EVERY;
        let Some(url) = self.telemetry_url.clone() else { return };
        let reports = self.telemetry.due(&shield::today(), &self.telemetry_snapshot());
        self.save();
        if reports.is_empty() {
            return;
        }
        self.telemetry_busy = true;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let days = reports
                .iter()
                .filter(|r| {
                    telemetry::send(&url, r)
                        .map_err(|e| log::warn!("статистика за {} не отправлена: {e}", r.day))
                        .is_ok()
                })
                .map(|r| r.day.clone())
                .collect();
            let _ = proxy.send_event(UserEvent::TelemetrySent { days });
        });
    }

    pub(super) fn on_telemetry_sent(&mut self, days: Vec<String>) {
        self.telemetry_busy = false;
        for d in &days {
            self.telemetry.sent(d);
        }
        self.save();
    }

    /// Пользователь включил или выключил статистику (Настройки, знакомство).
    pub(super) fn set_telemetry(&mut self, on: bool) {
        if on && self.telemetry_url.is_some() {
            self.telemetry.enable();
            self.telemetry.started(update::CURRENT, false, &shield::today());
            self.save();
        } else {
            // Номер установки, счётчики и неотправленное стираются с диска сразу.
            self.telemetry.disable();
        }
    }

    /// Для Настроек: есть ли статистика в этой сборке и каким будет сегодняшний отчёт.
    pub(super) fn telemetry_json(&self) -> Value {
        json!({
            "available": self.telemetry_url.is_some(),
            "preview": self.telemetry.preview(&self.telemetry_snapshot()),
        })
    }
}
