use std::time::Duration;

use helix_event::{register_hook, send_blocking};
use helix_view::handlers::{BlameEvent, Handlers};
use tokio::{task::JoinHandle, time::Instant};

use crate::{events::PostCommand, job};

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
            removed_lines_count,
            added_lines_count,
        } = event;

        self.cursor_line = cursor_line;

        // convert 0-based line numbers into 1-based line numbers
        let cursor_line = cursor_line + 1;

        // the line for which we compute the blame
        // Because gix_blame doesn't care about stuff that is not commited, we have to "normalize" the
        // line number to account for uncommited code.
        //
        // You'll notice that blame_line can be 0 when, for instance we have:
        // - removed 0 lines
        // - added 10 lines
        // - cursor_line is 8
        //
        // So when our cursor is on the 10th added line or earlier, blame_line will be 0. This means
        // the blame will be incorrect. But that's fine, because when the cursor_line is on some hunk,
        // we can show to the user nothing at all
        let blame_line = cursor_line.saturating_sub(added_lines_count) + removed_lines_count;

        let worker = tokio::spawn(async move {
            diff_providers
                .blame_line(&file, blame_line)
                .map(|s| s.to_string())
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
    register_hook!(move |event: &mut PostCommand<'_, '_>| {
        if !event.cx.editor.config().version_control.blame {
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

        let hunks = doc.diff_handle().unwrap().load();

        let mut removed_lines_count: u32 = 0;
        let mut added_lines_count: u32 = 0;
        for hunk in hunks.hunks_intersecting_line_ranges(std::iter::once((0, cursor_line as usize)))
        {
            let lines_inserted = hunk.after.end - hunk.after.start;
            let lines_removed = hunk.before.end - hunk.before.start;
            added_lines_count += lines_inserted;
            removed_lines_count += lines_removed;
        }

        send_blocking(
            &tx,
            BlameEvent::PostCommand {
                file,
                cursor_line,
                removed_lines_count,
                added_lines_count,
                // ok to clone because diff_providers is very small
                diff_providers: event.cx.editor.diff_providers.clone(),
            },
        );

        Ok(())
    });
}
