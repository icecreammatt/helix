#![allow(dead_code, unused_variables)]
use helix_event::{register_hook, send_blocking};
use helix_view::{
    handlers::{BlameEvent, Handlers},
    Editor,
};

use crate::{events::PostCommand, job};

pub struct BlameHandler;

impl helix_event::AsyncHook for BlameHandler {
    type Event = BlameEvent;

    fn handle_event(
        &mut self,
        event: Self::Event,
        timeout: Option<tokio::time::Instant>,
    ) -> Option<tokio::time::Instant> {
        self.finish_debounce();
        None
    }

    fn finish_debounce(&mut self) {
        job::dispatch_blocking(move |editor, _| {
            request_git_blame(editor);
        })
    }
}

pub(super) fn register_hooks(handlers: &Handlers) {
    let tx = handlers.blame.clone();
    register_hook!(move |event: &mut PostCommand<'_, '_>| {
        if event.cx.editor.config().vcs.blame {
            send_blocking(&tx, BlameEvent::PostCommand);
        }

        Ok(())
    });
}

fn request_git_blame(editor: &mut Editor) {
    let (view, doc) = current_ref!(editor);
    let text = doc.text();
    let selection = doc.selection(view.id);
    let Some(file) = doc.path() else {
        return;
    };
    let Ok(cursor_line) = TryInto::<u32>::try_into(
        text.char_to_line(selection.primary().cursor(doc.text().slice(..))),
    ) else {
        return;
    };

    let output = editor.diff_providers.blame_line(file, cursor_line);

    match output {
        Ok(blame) => editor.set_status(blame.to_string()),
        Err(err) => editor.set_error(err.to_string()),
    }
}
