//! First-dialog view. Message bodies and cursors live only in memory here.
use super::{
    connection::{Action, DeliveryJob, DialogScope, HistoryJob, Work},
    consent,
    send_intent::valid_text,
    send_recovery::{IpcResult, Progress, Projection, Submission},
};
use eframe::egui::{self, TextBuffer};
use kilogram_runtime_ipc::ordered2::{
    Delivery2, HistoryCursor2, HistoryPage2, HistoryRow2, MAX_HISTORY_ROWS, MessageError2,
};
use std::{
    collections::VecDeque,
    ops::Range,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

const TEXT_CAP: usize = kilogram_protocol::ordered_dialog_v2::MAX_TEXT;

#[derive(Default)]
struct Draft {
    text: Zeroizing<String>,
    revision: u64,
    rejected_edit: bool,
}
impl Draft {
    fn edit(&mut self, ui: &mut egui::Ui, slot: u8) {
        let previous = self.text.clone();
        let mut edit = BoundedEdit {
            text: &mut self.text,
            rejected: false,
        };
        let label = ui.label("Message draft");
        ui.add(
            egui::TextEdit::multiline(&mut edit)
                .id(editor_id(slot))
                .desired_width(f32::INFINITY)
                .desired_rows(3)
                .hint_text("Write a message"),
        )
        .labelled_by(label.id);
        if edit.rejected {
            self.text = previous;
            self.rejected_edit = true;
            clear_editor(ui.ctx(), slot);
        } else if self.text.as_str() != previous.as_str() {
            self.revision = self.revision.wrapping_add(1);
            self.rejected_edit = false;
        }
        if self.rejected_edit {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Edit exceeds 4096 bytes. Previous draft kept.",
            );
        }
    }
}

struct BoundedEdit<'a> {
    text: &'a mut String,
    rejected: bool,
}
impl TextBuffer for BoundedEdit<'_> {
    fn is_mutable(&self) -> bool {
        true
    }
    fn as_str(&self) -> &str {
        self.text
    }
    fn insert_text(&mut self, text: &str, char_index: usize) -> usize {
        if text.len() > TEXT_CAP.saturating_sub(self.text.len()) {
            self.rejected = true;
            return 0;
        }
        TextBuffer::insert_text(self.text, text, char_index)
    }
    fn delete_char_range(&mut self, chars: Range<usize>) {
        TextBuffer::delete_char_range(self.text, chars);
    }
    fn replace_with(&mut self, text: &str) {
        if text.len() > TEXT_CAP {
            self.rejected = true;
        } else {
            self.text.zeroize();
            self.text.push_str(text);
        }
    }
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<BoundedEdit<'static>>()
    }
}

fn editor_id(slot: u8) -> egui::Id {
    egui::Id::new(("first-dialog-draft", slot))
}

fn clear_editor(ctx: &egui::Context, slot: u8) {
    ctx.data_mut(|data| data.remove::<egui::text_edit::TextEditState>(editor_id(slot)));
}

struct SubmissionFlight {
    slot: u8,
    revision: u64,
    reply: Receiver<Submission>,
}

struct LoadedRow {
    data: HistoryRow2,
    delivery: Option<Delivery2>,
}
#[derive(Default)]
struct History {
    rows: VecDeque<LoadedRow>,
    cursor: Option<HistoryCursor2>,
    finished: bool,
    refresh_required: bool,
    error: Option<&'static str>,
}
impl History {
    fn apply(&mut self, page: HistoryPage2, replace: bool) -> bool {
        if !self.merge(page.items, replace) {
            return false;
        }
        self.finished = page.finished;
        self.cursor = Some(page.cursor);
        self.refresh_required = false;
        self.error = None;
        true
    }

    fn merge(&mut self, items: Vec<HistoryRow2>, replace: bool) -> bool {
        if !replace
            && items.iter().any(|row| {
                self.rows
                    .iter()
                    .find(|old| old.data.event == row.event)
                    .is_some_and(|old| {
                        old.data.request != row.request
                            || old.data.role != row.role
                            || old.data.ordinal != row.ordinal
                            || old.data.sequence != row.sequence
                            || old.data.text.as_str() != row.text.as_str()
                    })
            })
        {
            self.fail(MessageError2::InvalidReply);
            return false;
        }
        if replace {
            self.rows.clear();
        }
        for row in items {
            if self.rows.iter().any(|old| old.data.event == row.event) {
                continue;
            }
            if self.rows.len() == MAX_HISTORY_ROWS {
                self.rows.pop_front();
            }
            self.rows.push_back(LoadedRow {
                data: row,
                delivery: None,
            });
        }
        true
    }

    fn fail(&mut self, error: MessageError2) {
        if error == MessageError2::HistoryCursorInvalid {
            self.refresh_required = true;
            self.error = Some("This page can no longer be continued. Refresh history.");
        } else {
            self.error =
                Some("Could not load history. The previous page is still shown if available.");
        }
    }
}

struct PageFlight {
    scope: DialogScope,
    slot: u8,
    replace: bool,
    reply: Receiver<IpcResult<HistoryPage2>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct MessageKey {
    slot: u8,
    request: [u8; 16],
    event: [u8; 32],
}

struct DeliveryFlight {
    scope: DialogScope,
    key: MessageKey,
    reply: Receiver<Option<Delivery2>>,
}

pub(super) struct Dialog {
    drafts: [Draft; 2],
    slot: u8,
    scope: Option<DialogScope>,
    projection: Projection,
    submission: Option<SubmissionFlight>,
    history: History,
    page: Option<PageFlight>,
    delivery: Option<DeliveryFlight>,
    preview_delivery: Option<(MessageKey, Option<Delivery2>)>,
    next_lookup: Instant,
    round_robin: usize,
    notice: Option<&'static str>,
    reset_editors: bool,
}
impl Default for Dialog {
    fn default() -> Self {
        Self {
            drafts: Default::default(),
            slot: 0,
            scope: None,
            projection: Projection::default(),
            submission: None,
            history: History::default(),
            page: None,
            delivery: None,
            preview_delivery: None,
            next_lookup: Instant::now(),
            round_robin: 0,
            notice: None,
            reset_editors: false,
        }
    }
}

impl Dialog {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
        self.reset_editors = true;
    }

    fn prepare_editors(&mut self, ctx: &egui::Context) {
        if std::mem::take(&mut self.reset_editors) {
            clear_editor(ctx, 0);
            clear_editor(ctx, 1);
        }
    }

    pub(super) fn reset_runtime(&mut self) {
        self.scope = None;
        self.projection = Projection::default();
        self.preview_delivery = None;
        self.reset_history();
    }

    fn reset_history(&mut self) {
        self.history = History::default();
        self.page = None;
        self.delivery = None;
        self.round_robin = 0;
        self.next_lookup = Instant::now();
    }

    fn select(&mut self, slot: u8) {
        if slot != self.slot {
            self.slot = slot;
            self.reset_history();
        }
    }

    fn bind(&mut self, scope: DialogScope) {
        if self.scope != Some(scope) {
            self.reset_runtime();
            self.scope = Some(scope);
        }
    }

    fn request_send(&mut self, enqueue: impl FnOnce(Action) -> bool) -> bool {
        let draft = &self.drafts[usize::from(self.slot)];
        if self.submission.is_some() || !valid_text(&draft.text) {
            return false;
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        if !enqueue(Action::Send {
            slot: self.slot,
            text: draft.text.clone(),
            reply,
        }) {
            self.notice = Some("The local runtime is busy. Your draft is still here.");
            return false;
        }
        self.notice = None;
        self.submission = Some(SubmissionFlight {
            slot: self.slot,
            revision: draft.revision,
            reply: receiver,
        });
        true
    }

    /// The receipt belongs to a specific draft revision even if the selected slot changed.
    fn poll_submission(&mut self) -> Option<u8> {
        let flight = self.submission.as_ref()?;
        let result = match flight.reply.try_recv() {
            Err(TryRecvError::Empty) => return None,
            Ok(result) => result,
            Err(TryRecvError::Disconnected) => Submission::Retained,
        };
        let slot = flight.slot;
        let draft = &mut self.drafts[usize::from(slot)];
        let cleared = result == Submission::Protected && draft.revision == flight.revision;
        if cleared {
            draft.text.zeroize();
            draft.revision = draft.revision.wrapping_add(1);
            draft.rejected_edit = false;
        }
        if result == Submission::Retained {
            self.notice = Some(
                "The draft was kept. Check the local connection or saved send before trying again.",
            );
        }
        self.submission = None;
        cleared.then_some(slot)
    }

    fn request_page(&mut self, work: &mut Work, scope: DialogScope, replace: bool) {
        if self.page.is_some() {
            return;
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        let cursor = if replace {
            None
        } else {
            self.history.cursor.clone()
        };
        let job = HistoryJob {
            scope,
            slot: self.slot,
            cursor,
            reply,
        };
        if work.action(Action::History(Box::new(job))).is_ok() {
            self.page = Some(PageFlight {
                scope,
                slot: self.slot,
                replace,
                reply: receiver,
            });
        } else {
            self.history.fail(MessageError2::Unavailable);
        }
    }

    fn poll_page(&mut self) {
        let Some(flight) = self.page.as_ref() else {
            return;
        };
        let result = match flight.reply.try_recv() {
            Err(TryRecvError::Empty) => return,
            Ok(result) => result,
            Err(TryRecvError::Disconnected) => Err(MessageError2::Unavailable),
        };
        let valid = self.scope == Some(flight.scope) && flight.slot == self.slot;
        let replace = flight.replace;
        self.page = None;
        if !valid {
            return;
        }
        match result {
            Ok(page) => {
                if self.history.apply(page, replace) && replace {
                    self.delivery = None;
                    self.round_robin = 0;
                }
            }
            Err(error) => self.history.fail(error),
        }
    }

    fn poll_delivery(&mut self, preview: Option<MessageKey>) {
        let Some(flight) = self.delivery.as_ref() else {
            return;
        };
        let status = match flight.reply.try_recv() {
            Err(TryRecvError::Empty) => return,
            Ok(status) => status,
            Err(TryRecvError::Disconnected) => None,
        };
        let key = flight.key;
        let scope = flight.scope;
        self.delivery = None;
        if self.scope != Some(scope) {
            return;
        }
        if Some(key) == preview {
            self.preview_delivery = Some((key, status));
        }
        if self.slot == key.slot
            && let Some(row) = self.history.rows.iter_mut().find(|row| {
                row.data.role == scope.own_role
                    && row.data.request == key.request
                    && row.data.event == key.event
            })
        {
            row.delivery = status;
        }
    }

    fn schedule_delivery(&mut self, work: &Work, scope: DialogScope, keys: &[MessageKey]) {
        if self.delivery.is_some() || keys.is_empty() || Instant::now() < self.next_lookup {
            return;
        }
        let key = keys[self.round_robin % keys.len()];
        self.round_robin = self.round_robin.wrapping_add(1);
        let (reply, receiver) = mpsc::sync_channel(1);
        if work.check_delivery(DeliveryJob {
            scope,
            slot: key.slot,
            request: key.request,
            event: key.event,
            reply,
        }) {
            self.delivery = Some(DeliveryFlight {
                scope,
                key,
                reply: receiver,
            });
            self.next_lookup = Instant::now() + Duration::from_secs(1);
        }
    }

    pub(super) fn drain_submission(&mut self, ctx: &egui::Context) {
        if let Some(slot) = self.poll_submission() {
            clear_editor(ctx, slot);
        }
    }

    fn draw_slot_choice(&mut self, ui: &mut egui::Ui) {
        let mut selected = self.slot;
        ui.horizontal(|ui| {
            ui.radio_value(&mut selected, 0, "Conversation 1");
            ui.radio_value(&mut selected, 1, "Conversation 2");
        });
        self.select(selected);
    }

    pub(super) fn draw_offline(&mut self, ui: &mut egui::Ui) {
        self.prepare_editors(ui.ctx());
        self.drain_submission(ui.ctx());
        ui.collapsing("Draft a message", |ui| {
            self.draw_slot_choice(ui);
            self.drafts[usize::from(self.slot)].edit(ui, self.slot);
            ui.small(
                "Drafts stay in this window. Start the local connection to send or read history.",
            );
        });
    }

    pub(super) fn draw_online(&mut self, ui: &mut egui::Ui, work: &mut Work, connected: bool) {
        self.prepare_editors(ui.ctx());
        self.drain_submission(ui.ctx());
        if let Some(scope) = work.view.messages.scope {
            self.bind(scope);
            self.projection = work.view.messages.pending.clone();
        }
        ui.separator();
        ui.heading("Messages");
        ui.small("Choose one of two conversations with the same peer.");
        self.draw_slot_choice(ui);
        self.poll_page();
        let scope = self.scope;
        let projection = self.projection.clone();
        let item = projection.item.as_deref().filter(|item| {
            scope.and_then(|scope| scope.contexts.get(usize::from(item.slot)).copied())
                == Some(item.context)
        });
        let preview = item.and_then(|item| {
            Some(MessageKey {
                slot: item.slot,
                request: item.request,
                event: projection.event?,
            })
        });
        self.poll_delivery(preview);
        let mut keys = Vec::new();
        let ready = work.view.running
            && work.view.request_started.elapsed() < Duration::from_secs(2)
            && consent::now().is_ok_and(|wall| wall < work.view.consent_expires);

        let merged =
            if let (Some(scope), Some(item), Some(key)) = (scope, item, preview) {
                item.slot == self.slot
                    && self.history.rows.iter().any(|row| {
                        matches_preview(&row.data, scope.own_role, key, item.text.as_str())
                    })
            } else {
                false
            };
        if let Some(item) = item
            && !merged
        {
            ui.group(|ui| {
                let heading = if projection.busy {
                    "Saved send"
                } else {
                    "Last send"
                };
                ui.strong(format!("{heading} · Conversation {}", item.slot + 1));
                egui::ScrollArea::vertical()
                    .id_salt("saved-send-text")
                    .max_height(65.0)
                    .show(ui, |ui| {
                        ui.label(item.text.as_str());
                    });
                let delivery = preview
                    .and_then(|key| self.preview_delivery.filter(|(old, _)| *old == key))
                    .map(|(_, delivery)| delivery);
                let label = if projection.busy {
                    projection
                        .progress
                        .notice()
                        .unwrap_or("The saved send is awaiting confirmation.")
                } else if let Some(delivery) = delivery {
                    delivery_label(delivery, connected)
                } else if projection.progress == Progress::Delivered {
                    delivery_label(Some(Delivery2::Delivered), connected)
                } else {
                    delivery_label(Some(Delivery2::Materialized), connected)
                };
                let response = ui.label(label);
                if !projection.busy
                    && projection.progress != Progress::Delivered
                    && delivery != Some(Some(Delivery2::Delivered))
                    && response.rect.intersects(ui.clip_rect())
                    && let Some(key) = preview
                {
                    keys.push(key);
                }
                if projection.progress == Progress::RetryAvailable
                    && ui
                        .add_enabled(
                            ready
                                && self.submission.is_none()
                                && work.view.messages.send_slots[usize::from(item.slot)],
                            egui::Button::new("Retry this saved send"),
                        )
                        .clicked()
                    && work.action(Action::RetrySend).is_err()
                {
                    self.notice = Some("The local runtime is busy. The saved send was kept.");
                }
            });
        } else if projection.busy
            && projection.progress != Progress::Empty
            && let Some(notice) = projection.progress.notice()
        {
            ui.label(notice);
        }

        let draft = &mut self.drafts[usize::from(self.slot)];
        draft.edit(ui, self.slot);
        let (draft_bytes, draft_valid) = (draft.text.len(), valid_text(&draft.text));
        ui.horizontal(|ui| {
            let enabled = ready
                && work.view.messages.send_slots[usize::from(self.slot)]
                && !projection.busy
                && self.submission.is_none()
                && draft_valid;
            let label = if connected {
                "Send message"
            } else {
                "Queue message locally"
            };
            let clicked = ui.add_enabled(enabled, egui::Button::new(label)).clicked();
            ui.small(format!("{draft_bytes} / 4096 bytes"));
            if clicked {
                self.request_send(|action| work.action(action).is_ok());
            }
        });
        if self.submission.is_some() {
            ui.small("Saving this send…");
        } else if projection.busy {
            ui.small("One saved send at a time across both conversations.");
        } else if !ready {
            ui.small("Wait for the local runtime before sending.");
        }
        if let Some(notice) = self.notice {
            ui.label(notice);
        }

        ui.horizontal(|ui| {
            let available = work.view.running && self.page.is_none();
            if ui
                .add_enabled(available, egui::Button::new("Refresh history"))
                .clicked()
                && let Some(scope) = scope
            {
                self.request_page(work, scope, true);
            }
            if ui
                .add_enabled(
                    available
                        && self.history.cursor.is_some()
                        && !self.history.finished
                        && !self.history.refresh_required,
                    egui::Button::new("Next history page"),
                )
                .clicked()
                && let Some(scope) = scope
            {
                self.request_page(work, scope, false);
            }
        });
        if let Some(error) = self.history.error {
            ui.label(error);
        } else if self.page.is_some() {
            ui.small("Loading history…");
        } else if self.history.cursor.is_none() {
            ui.small("Refresh starts at the beginning of the saved history.");
        } else if self.history.finished {
            ui.small("End of this history snapshot.");
        } else {
            ui.small("More messages are available in this snapshot.");
        }
        if let Some(scope) = scope {
            egui::ScrollArea::vertical()
                .id_salt(("first-dialog-history", self.slot))
                .max_height(175.0)
                .show(ui, |ui| {
                    for row in &self.history.rows {
                        let own = row.data.role == scope.own_role;
                        let key = MessageKey {
                            slot: self.slot,
                            request: row.data.request,
                            event: row.data.event,
                        };
                        let merged_delivery = if Some(key) == preview
                            && item.is_some_and(|item| {
                                matches_preview(&row.data, scope.own_role, key, item.text.as_str())
                            }) {
                            if projection.progress == Progress::Delivered {
                                Some(Delivery2::Delivered)
                            } else {
                                self.preview_delivery
                                    .filter(|(old, _)| *old == key)
                                    .and_then(|(_, delivery)| delivery)
                            }
                        } else {
                            None
                        };
                        let delivered = row.delivery == Some(Delivery2::Delivered)
                            || merged_delivery == Some(Delivery2::Delivered);
                        let response = ui
                            .group(|ui| {
                                ui.strong(if own { "You" } else { "Peer" });
                                ui.label(row.data.text.as_str());
                                if own {
                                    let delivery = if delivered {
                                        Some(Delivery2::Delivered)
                                    } else {
                                        row.delivery.or(merged_delivery)
                                    };
                                    ui.small(delivery_label(delivery, connected));
                                }
                            })
                            .response;
                        if own && !delivered && response.rect.intersects(ui.clip_rect()) {
                            keys.push(key);
                        }
                    }
                });
            if work.view.running {
                self.schedule_delivery(work, scope, &keys);
            }
        }
    }
}

fn matches_preview(row: &HistoryRow2, own_role: u8, key: MessageKey, text: &str) -> bool {
    row.role == own_role
        && row.request == key.request
        && row.event == key.event
        && row.text.as_str() == text
}

fn delivery_label(delivery: Option<Delivery2>, connected: bool) -> &'static str {
    match delivery {
        Some(Delivery2::Delivered) if connected => "Delivered",
        Some(Delivery2::Delivered) => "Delivered earlier",
        Some(Delivery2::Queued) => "In local queue",
        Some(Delivery2::Materialized) => "Saved in history · delivery not confirmed",
        None => "Delivery not confirmed",
    }
}

#[cfg(test)]
mod tests;
