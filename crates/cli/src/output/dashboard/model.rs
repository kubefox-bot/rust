use super::super::console;
use hmail_cleaner_core::cleanup::{Stage, Summary};
use std::{collections::VecDeque, time::Instant};
const HISTORY: usize = 5;
const MIN_BAR_WIDTH: usize = 5;
const MAX_BAR_WIDTH: usize = 50;
const BAR_LABEL_WIDTH: usize = 30;
pub(in crate::output) struct Model {
    pub title: String,
    pub context: Vec<String>,
    pub preview: bool,
    pub service: String,
    pub stage: Stage,
    pub summary: Summary,
    pub started: Instant,
    pub changed: Instant,
    pub history: VecDeque<String>,
}
impl Model {
    pub(in crate::output) fn note(&mut self, message: String) {
        if self.history.len() == HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(format!(
            "{}  {}",
            chrono::Local::now().format("%H:%M:%S"),
            console::redact(&message)
        ));
    }
    pub(super) fn lines(&self, width: usize, height: usize, tick: u64) -> Vec<String> {
        let s = self.summary;
        let mut header = vec![self.title.clone(), "-".repeat(width)];
        header.extend(self.context.iter().cloned());
        header.push(format!("Почтовая служба: {}", self.service));
        let mut lines = Vec::new();
        lines.push(format!("Сейчас: {}", console::stage(self.stage)));
        lines.push(format!(
            "Обход: {} файлов | сверка: {} | записей БД: {}",
            s.scanned_files, s.verified_files, s.examined_messages
        ));
        lines.push(if self.preview {
            format!(
                "К удалению: {} файлов | отсутствовало: {} | объём: {:.2} ГиБ",
                s.selected_files,
                s.absent_files,
                s.selected_bytes as f64 / super::super::BYTES_PER_GIB
            )
        } else {
            format!(
                "Удалено файлов: {} | освобождено: {:.2} ГиБ | отсутствовало: {}",
                s.removed_files,
                s.removed_bytes as f64 / super::super::BYTES_PER_GIB,
                s.absent_files
            )
        });
        lines.push(format!(
            "Удалено записей: {} | метаданных: {} | получателей: {}",
            s.deleted_messages,
            s.deleted_metadata + s.orphan_metadata,
            s.deleted_recipients + s.orphan_recipients
        ));
        lines.push(format!(
            "Ошибок: {} | папок удалено: {} | старых файлов осталось: {}",
            s.failed,
            s.removed_directories,
            s.remaining_old_files
                .map(|count| count.to_string())
                .unwrap_or_else(|| "не проверено".into())
        ));
        let elapsed = self.started.elapsed();
        let seconds = elapsed.as_secs();
        let spinner = ["|", "/", "-", "\\"][(tick % 4) as usize];
        let bar_width = width
            .saturating_sub(BAR_LABEL_WIDTH)
            .clamp(MIN_BAR_WIDTH, MAX_BAR_WIDTH);
        let bar = if self.stage == Stage::Scan {
            match s.total_scan_files {
                Some(0) => format!("[{}] файлов для обхода нет", "-".repeat(bar_width)),
                Some(total) => {
                    let processed = s.scanned_files.min(total);
                    let filled = (processed.saturating_mul(bar_width as u64) / total) as usize;
                    format!(
                        "[{}{}] {}% — обход",
                        "#".repeat(filled),
                        "-".repeat(bar_width - filled),
                        processed.saturating_mul(100) / total
                    )
                }
                None => moving_bar(bar_width, tick),
            }
        } else {
            moving_bar(bar_width, tick)
        };
        let footer = vec![
            "-".repeat(width),
            format!(
                "{spinner} Работаем | {:02}:{:02}:{:02} | {:.0} файлов/с",
                seconds / 3600,
                (seconds / 60) % 60,
                seconds % 60,
                s.scanned_files as f64 / elapsed.as_secs_f64().max(1.0)
            ),
            format!(
                "Последний прогресс: {} с назад",
                self.changed.elapsed().as_secs()
            ),
            bar,
            "Ctrl+C — остановить после текущего пакета".into(),
        ];
        let body_height = height.saturating_sub(footer.len());
        let extras = body_height.saturating_sub(lines.len());
        let mut body = header.into_iter().take(extras).collect::<Vec<_>>();
        let history_rows = extras.saturating_sub(body.len());
        if history_rows > 0 {
            body.push("ДЕЙСТВИЯ ПРОГРАММЫ".into());
            let count = history_rows.saturating_sub(1).min(self.history.len());
            body.extend(
                self.history
                    .iter()
                    .skip(self.history.len() - count)
                    .cloned(),
            );
        }
        body.resize(body_height.saturating_sub(lines.len()), String::new());
        body.extend(lines);
        body.truncate(body_height);
        let mut lines = body;
        lines.extend(footer);
        lines.truncate(height);
        lines
            .into_iter()
            .map(|line| line.chars().take(width).collect())
            .collect()
    }
}
fn moving_bar(width: usize, tick: u64) -> String {
    let position = tick as usize % width;
    format!(
        "[{}>{}] объём неизвестен",
        "-".repeat(position),
        "-".repeat(width - position - 1)
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> Model {
        Model {
            title: "Тест".into(),
            context: vec![],
            preview: false,
            service: "проверяется".into(),
            stage: Stage::Scan,
            summary: Summary::default(),
            started: Instant::now(),
            changed: Instant::now(),
            history: VecDeque::new(),
        }
    }
    #[test]
    fn animation_advances_without_progress_and_respects_window() {
        let mut state = model();
        assert_ne!(state.lines(80, 25, 0), state.lines(80, 25, 1));
        for n in 0..10 {
            state.note(n.to_string());
        }
        assert_eq!(state.history.len(), 5);
        assert!(state
            .lines(30, 10, 0)
            .iter()
            .all(|l| l.chars().count() <= 30));
        assert_eq!(state.lines(30, 10, 0).len(), 10);
        assert!(state.lines(30, 10, 0).join("\n").contains("Сейчас:"));
        assert!(state.lines(30, 10, 0).join("\n").contains("Ошибок:"));
    }
    #[test]
    fn percent_is_only_shown_for_known_scan_total() {
        let mut state = model();
        assert!(!state.lines(80, 25, 0).join("\n").contains("%"));
        state.summary.total_scan_files = Some(100);
        state.summary.scanned_files = 50;
        assert!(state.lines(80, 25, 0).join("\n").contains("50%"));
        state.stage = Stage::Database;
        assert!(!state.lines(80, 25, 0).join("\n").contains("50%"));
    }
}
