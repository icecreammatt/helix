use std::time::Duration;

use helix_event::{register_hook, send_blocking};
use helix_vcs::FileBlame;
use helix_view::{
    events::DidRequestFileBlameUpdate,
    handlers::{BlameEvent, Handlers},
    DocumentId,
};
use tokio::{task::JoinHandle, time::Instant};

use crate::{events::DidRequestInlineBlameUpdate, job};

#[derive(Default)]
pub struct BlameHandler {
    worker: Option<JoinHandle<anyhow::Result<FileBlame>>>,
    doc_id: DocumentId,
}

impl helix_event::AsyncHook for BlameHandler {
    type Event = BlameEvent;

    fn handle_event(
        &mut self,
        event: Self::Event,
        _timeout: Option<tokio::time::Instant>,
    ) -> Option<tokio::time::Instant> {
        if let Some(worker) = &self.worker {
            if worker.is_finished() {
                self.finish_debounce();
                return None;
            }
            return Some(Instant::now() + Duration::from_millis(50));
        }

        self.doc_id = event.doc_id;

        let worker = tokio::spawn(async move { FileBlame::try_new(event.path) });
        self.worker = Some(worker);
        Some(Instant::now() + Duration::from_millis(50))
    }

    fn finish_debounce(&mut self) {
        let doc_id = self.doc_id;
        if let Some(worker) = &self.worker {
            if worker.is_finished() {
                let worker = self.worker.take().expect("Inside of an if let Some(...)");
                tokio::spawn(async move {
                    let Ok(Ok(file_blame)) = worker.await else {
                        return;
                    };
                    job::dispatch(move |editor, _| {
                        let Some(doc) = editor.document_mut(doc_id) else {
                            return;
                        };
                        doc.file_blame = Some(file_blame);
                    })
                    .await;
                });
            }
        }
    }
}

pub(super) fn register_hooks(handlers: &Handlers) {
    let tx = handlers.blame.clone();
    register_hook!(move |event: &mut DidRequestFileBlameUpdate<'_>| {
        let version_control_config = &event.editor.config().version_control;
        if !version_control_config.inline_blame {
            return Ok(());
        }

        let Some(doc) = event.editor.document(event.doc) else {
            return Ok(());
        };

        let Some(path) = doc.path() else {
            return Ok(());
        };

        send_blocking(
            &tx,
            BlameEvent {
                path: path.to_path_buf(),
                doc_id: event.doc,
            },
        );

        Ok(())
    });
    register_hook!(move |event: &mut DidRequestInlineBlameUpdate<'_>| {
        let version_control_config = &event.editor.config().version_control;
        let (view, doc) = current!(event.editor);

        if !version_control_config.inline_blame {
            return Ok(());
        }

        let cursor_line = doc.cursor_line(view.id);
        let Some(diff_handle) = doc.diff_handle() else {
            return Ok(());
        };
        let (inserted_lines, deleted_lines) = diff_handle
            .load()
            .inserted_and_deleted_before_line(cursor_line);

        let Some(blame) = &doc.file_blame else {
            return Ok(());
        };

        let blame = blame
            .blame_for_line(cursor_line as u32, inserted_lines, deleted_lines)
            .parse_format(&version_control_config.inline_blame_format);

        doc.blame = Some(blame);

        Ok(())
    });
}
