use std::time::Duration;

use helix_event::{register_hook, send_blocking};
use helix_view::handlers::{BlameEvent, Handlers};
use tokio::{task::JoinHandle, time::Instant};

use crate::{events::PostCommand, job};

#[derive(Default)]
pub struct BlameHandler {
    worker: Option<JoinHandle<anyhow::Result<String>>>,
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

        let BlameEvent::PostCommand {
            file,
            cursor_line,
            diff_providers,
        } = event;

        let worker = tokio::spawn(async move {
            diff_providers
                .blame_line(&file, cursor_line)
                .map(|s| s.to_string())
        });
        self.worker = Some(worker);
        Some(Instant::now() + Duration::from_millis(50))
    }

    fn finish_debounce(&mut self) {
        if let Some(worker) = &self.worker {
            if worker.is_finished() {
                let worker = self.worker.take().unwrap();
                tokio::spawn(async move {
                    let Ok(Ok(outcome)) = worker.await else {
                        return;
                    };
                    job::dispatch(move |editor, _| {
                        let doc = doc_mut!(editor);
                        doc.blame = Some(outcome);
                    })
                    .await;
                });
            }
        }
    }
}

pub(super) fn register_hooks(handlers: &Handlers) {
    let tx = handlers.blame.clone();
    register_hook!(move |event: &mut PostCommand<'_, '_>| {
        if !event.cx.editor.config().vcs.blame {
            return Ok(());
        }

        let (view, doc) = current!(event.cx.editor);
        let text = doc.text();
        let selection = doc.selection(view.id);
        let Some(file) = doc.path() else {
            return Ok(());
        };
        let file = file.to_path_buf();

        let Ok(cursor_line) =
            u32::try_from(text.char_to_line(selection.primary().cursor(doc.text().slice(..))))
        else {
            return Ok(());
        };

        send_blocking(
            &tx,
            BlameEvent::PostCommand {
                file,
                cursor_line,
                // ok to clone because diff_providers is very small
                diff_providers: event.cx.editor.diff_providers.clone(),
            },
        );

        Ok(())
    });
}
