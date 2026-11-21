//! The product follows authenticated O(new rows) history on an independent,
//! pinned read-only IPC attachment. Display checkpoints advance after stdout
//! flush, never merely after reading a page. No owner/descriptor adoption.
use super::*;
use kilogram_first_dialog::{
    chat::{Chat, Mode},
    operation::{OperationControl, OperationKind},
};
use kilogram_runtime_ipc::ordered2::{HistoryCursor2, MessageError2};
use std::time::Instant;

#[derive(Default)]
struct Position {
    cursor: Option<HistoryCursor2>,
    failed: bool,
}
fn control() -> Arc<OperationControl> {
    Arc::new(OperationControl::new(
        OperationKind::Read,
        Instant::now() + Duration::from_secs(2),
        None,
    ))
}
#[derive(Default)]
struct Counts {
    positions: u64,
    pages: u64,
    rows: u64,
}
impl Drop for Counts {
    fn drop(&mut self) {
        if cfg!(debug_assertions)
            && std::env::var("KILOGRAM_TEST_OD2_TIMINGS").is_ok_and(|s| s == "1")
            && let Some(dir) = std::env::var_os("KILOGRAM_TEST_OD2_TIMING_DIRECTORY")
        {
            let path = PathBuf::from(dir).join(format!("reader-{}.json", std::process::id()));
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                let _ = writeln!(
                    f,
                    "{}",
                    json!({"positions":self.positions,"pages":self.pages,"rows":self.rows})
                );
            }
        }
    }
}

pub(super) type Presence = Arc<Mutex<Option<(Instant, Value)>>>;
pub(super) fn run(
    home: PathBuf,
    instance: String,
    ui: Ui,
    replay: Arc<AtomicBool>,
    presence: Presence,
) -> Result<()> {
    let mut chat = Chat::new(home.clone(), std::env::current_exe()?);
    let status = chat.connect_controlled(
        Mode::Observer,
        Some(&instance),
        Some(&control()),
        &Default::default(),
    )?;
    let mut counts = Counts::default();
    let mut next_status = Instant::now();
    let mut previous_presence = None;
    let paths = status
        .contexts
        .each_ref()
        .map(|context| home.join(format!("peer-history-{context}.cursor")));
    let mut positions: [Position; 2] = std::array::from_fn(|slot| {
        if paths[slot].exists() {
            let result = model::read(&paths[slot], 367)
                .and_then(|raw| Ok(HistoryCursor2::from_checkpoint(std::str::from_utf8(&raw)?)?));
            match result {
                Ok(cursor) => Position {
                    cursor: Some(cursor),
                    failed: false,
                },
                Err(_) => {
                    ui.emit("history_gap","Позиция отображения повреждена. История сохранена; /history — прочитать, /resync — повторно показать сохранённое.",json!({"code":"checkpoint_format_invalid"}));
                    Position {
                        cursor: None,
                        failed: true,
                    }
                }
            }
        } else {
            Position::default()
        }
    });
    while !ui.stop.load(Ordering::Acquire) {
        if Instant::now() >= next_status {
            if let Ok(status) = chat.status_controlled(Some(&control())) {
                let value = json!({"status":status});
                let observed = (
                    value["status"]["live_admitted"].clone(),
                    value["status"]["selected_routes"].clone(),
                );
                if previous_presence.as_ref() != Some(&observed) {
                    ui.emit(
                        "connection",
                        terminal::connection_notice(&value),
                        value.clone(),
                    );
                    previous_presence = Some(observed);
                }
                if let Ok(mut current) = presence.lock() {
                    *current = Some((Instant::now(), value));
                }
            }
            next_status = Instant::now() + Duration::from_secs(1);
        }
        if replay.swap(false, Ordering::AcqRel) {
            // An explicit display replay changes no message/account state.
            positions = std::array::from_fn(|_| Position::default());
        }
        for slot in 0..2 {
            if ui.stop.load(Ordering::Acquire) || positions[slot].failed {
                continue;
            }
            let result = (|| -> Result<()> {
                let position = &mut positions[slot];
                if let Some(cursor) = position.cursor.as_ref().filter(|c| c.completed()) {
                    counts.positions += 1;
                    let next = chat.history_position_controlled(
                        slot as u8,
                        Some(cursor.clone()),
                        &control(),
                    )?;
                    if next.next == next.frontiers {
                        return Ok(());
                    }
                    position.cursor = Some(next.cursor);
                }
                // Initial open reads the actual snapshot from ordinal 1, rather
                // than anchoring past messages which raced startup. Thereafter
                // saved authenticated cursors prevent repeated full scans.
                for _ in 0..8 {
                    if ui.stop.load(Ordering::Acquire) {
                        break;
                    }
                    counts.pages += 1;
                    let page = chat.history_controlled(
                        slot as u8,
                        position.cursor.clone(),
                        16,
                        Some(&control()),
                    )?;
                    for row in &page.items {
                        if ui.stop.load(Ordering::Acquire) {
                            return Ok(());
                        }
                        counts.rows += 1;
                        if row.role!=status.own_roles[slot] && !ui.emitted("message",&format!("Собеседник: {}",row.text.as_str()),json!({"context":status.contexts[slot],"event_id":row.event.iter().map(|b|format!("{b:02x}")).collect::<String>(),"text":row.text.as_str()})) {
                            bail!("Terminal output not confirmed; display position preserved")
                        }
                    }
                    // A crash between output and checkpoint may repeat a page;
                    // it cannot silently skip an unflushed incoming message.
                    atomic(&paths[slot], page.cursor.checkpoint().as_bytes())?;
                    let finished = page.finished;
                    position.cursor = Some(page.cursor);
                    if finished {
                        break;
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                if matches!(
                    error.downcast_ref::<MessageError2>(),
                    Some(MessageError2::HistoryUnavailable | MessageError2::Unavailable)
                ) {
                    continue; // No cursor/output is advanced; bounded IPC retries.
                }
                positions[slot].failed = true;
                let code = if matches!(
                    error.downcast_ref::<MessageError2>(),
                    Some(MessageError2::HistoryCursorInvalid)
                ) {
                    "history_cursor_invalid"
                } else {
                    "history_read_unconfirmed"
                };
                ui.emit("history_gap","Автоматическое чтение истории не подтверждено. Сообщения сохранены; /history — проверить, /resync — повторно показать сохранённое.",json!({"code":code}));
            }
        }
        // Cancel promptly on ordinary quit; each IPC itself is deadline-bound.
        for _ in 0..10 {
            if ui.stop.load(Ordering::Acquire) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    chat.disconnect()?;
    Ok(())
}
