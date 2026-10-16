use super::event::Event;
use crate::{args::Mode, commands::Status};
use hmail_cleaner_core::cleanup::{Failure, Stage, Summary};
use std::io::{self, Write};
pub(super) fn stage(stage: Stage) -> &'static str {
    match stage {
        Stage::Preflight => "Проверка настроек и почтовой службы",
        Stage::Snapshot => "Чтение снимка базы данных",
        Stage::Selection => "Отбор файлов по дате создания",
        Stage::Files => "Удаление файлов и подтверждение отсутствия",
        Stage::Database => "Удаление связанных записей базы данных",
        Stage::Verification => "Контрольная сверка",
        Stage::Directories => "Удаление пустых папок",
        Stage::Complete => "Обработка завершена; восстановление службы и закрытие БД",
        Stage::Cancelled => "Остановка после завершения текущего пакета",
        Stage::OrphanDatabase => "Очистка записей без родительского письма",
        Stage::References => "Загрузка карты путей из базы данных",
        Stage::Scan => "Рекурсивный обход файлов",
    }
}
pub(super) fn counters(s: &Summary, preview: bool) -> String {
    format!("проверено записей: {}; проверено файлов: {}; выбрано записей: {}; {}: {}; {}: {:.2} ГиБ; отсутствовало: {}; удалено записей БД: {}; метаданных: {}; получателей: {}; бесхозных файлов выбрано/удалено: {}/{}; бесхозных метаданных выбрано/удалено: {}/{}; бесхозных получателей выбрано/удалено: {}/{}; пустых папок удалено: {}; старых файлов осталось: {}; ошибок: {}",
        s.examined_messages,s.scanned_files,s.selected,
        if preview {"файлов к удалению"} else {"файлов удалено"},if preview {s.selected_files} else {s.removed_files},
        if preview {"потенциальный объём"} else {"освобождено"},if preview {s.selected_bytes} else {s.removed_bytes} as f64/super::BYTES_PER_GIB,
        s.absent_files,s.deleted_messages,s.deleted_metadata,s.deleted_recipients,s.selected_orphan_files,s.removed_orphan_files,s.selected_orphan_metadata,s.orphan_metadata,s.selected_orphan_recipients,s.orphan_recipients,s.removed_directories,s.remaining_old_files.map(|count|count.to_string()).unwrap_or_else(||"не проверено".into()),s.failed)
}
pub(super) fn text(event: &Event<'_>, preview: bool) -> String {
    match event {
        Event::Action{message} => message.to_string(),
        Event::Server{mysql_version,cutoff} => format!("MySQL {mysql_version}; дата создания файла строго раньше {cutoff} (местное время); для отсутствующих файлов — дата БД"),
        Event::Progress(p) => match &p.failure {
            Some(failure) => format!("{}: {}",stage(p.stage),match failure {
                Failure::Message{id,reason} => format!("Ошибка записи {id}: {reason}"),
                Failure::Directory{path,reason} => format!("Ошибка папки {}: {reason}",path.display()),
                Failure::OrphanFile{path,reason} => format!("Ошибка файла {}: {reason}",path.display()),
            }),
            None => format!("{}: {}",stage(p.stage),counters(&p.summary,preview)),
        },
        Event::Result(r) => format!("{} — {}: {}{}",
            match r.mode {Some(Mode::Preview)=>"Предпросмотр",Some(Mode::Clean)=>"Очистка",None=>"Запуск"},
            match r.status {Status::Complete=>"завершено",Status::Partial=>"завершено с ошибками",Status::Failed=>"ошибка",Status::Invalid=>"ошибка настроек",Status::Cancelled=>"отменено"},
            counters(&r.summary,r.mode==Some(Mode::Preview)),r.error.as_ref().map(|e|format!("; Причина: {e}")).unwrap_or_default()),
    }
}
pub(super) fn redact(text: &str) -> String {
    let mut safe = text.to_owned();
    while let Some(start) = safe.find("mysql://") {
        let end = safe[start..]
            .find(char::is_whitespace)
            .map(|n| start + n)
            .unwrap_or(safe.len());
        safe.replace_range(start..end, "[строка подключения скрыта]");
    }
    safe
}
pub(super) fn write(event: &Event<'_>, preview: bool) -> io::Result<()> {
    let text = redact(&text(event, preview));
    if matches!(event, Event::Result(_)) {
        writeln!(io::stdout().lock(), "{text}")
    } else {
        writeln!(io::stderr().lock(), "{text}")
    }
}
