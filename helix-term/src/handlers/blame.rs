use std::{path::PathBuf, time::Duration};

use helix_event::{register_hook, send_blocking};
use helix_vcs::DiffProviderRegistry;
use helix_view::handlers::{BlameEvent, Handlers};
use tokio::{task::JoinHandle, time::Instant};

use crate::{events::PostCommand, job};

#[derive(Default)]
pub struct BlameHandler {
    worker: Option<JoinHandle<anyhow::Result<String>>>,
}

async fn compute_diff(
    file: PathBuf,
    line: u32,
    diff_providers: DiffProviderRegistry,
) -> anyhow::Result<String> {
    // std::thread::sleep(Duration::from_secs(5));
    // Ok("hhe".to_string())
    diff_providers
        .blame_line(&file, line)
        .map(|s| s.to_string())
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

        let worker = tokio::spawn(compute_diff(file, cursor_line, diff_providers));
        self.worker = Some(worker);
        Some(Instant::now() + Duration::from_millis(50))
    }

    fn finish_debounce(&mut self) {
        if let Some(worker) = &self.worker {
            if worker.is_finished() {
                let worker = self.worker.take().unwrap();
                tokio::spawn(handle_worker(worker));
            }
        }
    }
}

async fn handle_worker(worker: JoinHandle<anyhow::Result<String>>) {
    let Ok(Ok(outcome)) = worker.await else {
        return;
    };
    job::dispatch(move |editor, _| {
        let doc = doc_mut!(editor);
        doc.blame = Some(outcome);
    })
    .await;
}

pub(super) fn register_hooks(handlers: &Handlers) {
    let tx = handlers.blame.clone();
    register_hook!(move |event: &mut PostCommand<'_, '_>| {
        if event.cx.editor.config().vcs.blame {
            let (view, doc) = current!(event.cx.editor);
            let text = doc.text();
            let selection = doc.selection(view.id);
            let Some(file) = doc.path() else {
                return Ok(());
            };

            let Ok(cursor_line) = TryInto::<u32>::try_into(
                text.char_to_line(selection.primary().cursor(doc.text().slice(..))),
            ) else {
                return Ok(());
            };

            send_blocking(
                &tx,
                BlameEvent::PostCommand {
                    file: file.to_path_buf(),
                    cursor_line,
                    diff_providers: event.cx.editor.diff_providers.clone(),
                },
            );
        }

        Ok(())
    });
}
