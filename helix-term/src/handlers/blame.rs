use std::time::Duration;

use helix_event::{register_hook, send_blocking};
use helix_view::{
    events::DidRequestInlineBlame,
    handlers::{BlameEvent, Handlers},
};
use tokio::{task::JoinHandle, time::Instant};

use crate::job;

#[derive(Default)]
pub struct BlameHandler {
    worker: Option<JoinHandle<anyhow::Result<String>>>,
    cursor_line: u32,
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
            deleted_lines_count: removed_lines_count,
            inserted_lines_count: added_lines_count,
            blame_format,
        } = event;

        self.cursor_line = cursor_line;

        let worker = tokio::spawn(async move {
            diff_providers
                .blame_line(&file, cursor_line, added_lines_count, removed_lines_count)
                .map(|s| s.parse_format(&blame_format))
        });
        self.worker = Some(worker);
        Some(Instant::now() + Duration::from_millis(50))
    }

    fn finish_debounce(&mut self) {
        let cursor_line = self.cursor_line;
        if let Some(worker) = &self.worker {
            if worker.is_finished() {
                let worker = self.worker.take().unwrap();
                tokio::spawn(async move {
                    let Ok(Ok(outcome)) = worker.await else {
                        return;
                    };
                    job::dispatch(move |editor, _| {
                        let doc = doc_mut!(editor);
                        // if we're on a line that hasn't been commited yet, just show nothing at all
                        // in order to reduce visual noise.
                        // Because the git hunks already imply this information
                        let blame_text = doc
                            .diff_handle()
                            .is_some_and(|diff| diff.load().hunk_at(cursor_line, false).is_none())
                            .then_some(outcome);
                        doc.blame = blame_text;
                    })
                    .await;
                });
            }
        }
    }
}

pub(super) fn register_hooks(handlers: &Handlers) {
    let tx = handlers.blame.clone();
    register_hook!(move |event: &mut DidRequestInlineBlame<'_>| {
        let version_control_config = &event.editor.config().version_control;
        if !version_control_config.inline_blame {
            return Ok(());
        }

        let (view, doc) = current!(event.editor);

        let Some(file) = doc.path() else {
            return Ok(());
        };
        let file = file.to_path_buf();

        let Ok(cursor_line) = u32::try_from(doc.cursor_line(view.id)) else {
            return Ok(());
        };

        if let Some(cached) = &mut event.editor.blame_cache {
            // don't update the blame if we haven't moved to a different line
            if (view.id, cursor_line) == *cached {
                return Ok(());
            } else {
                *cached = (view.id, cursor_line)
            }
        };

        let Some(hunks) = doc.diff_handle() else {
            return Ok(());
        };

        log::error!("updated blame!");

        let (inserted_lines_count, deleted_lines_count) = hunks
            .load()
            .inserted_and_deleted_before_line(cursor_line as usize);

        send_blocking(
            &tx,
            BlameEvent::PostCommand {
                file,
                cursor_line,
                deleted_lines_count,
                inserted_lines_count,
                // ok to clone because diff_providers is very small
                diff_providers: event.editor.diff_providers.clone(),
                // ok to clone because blame_format is likely to be about 30 characters or less
                blame_format: version_control_config.inline_blame_format.clone(),
            },
        );

        Ok(())
    });
}
