use helix_core::text_annotations::InlineAnnotation;
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
        _event: Self::Event,
        _timeout: Option<tokio::time::Instant>,
    ) -> Option<tokio::time::Instant> {
        self.finish_debounce();
        None
    }

    fn finish_debounce(&mut self) {
        // TODO: this blocks on the main thread. Figure out how not to do that
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
    let blame_enabled = editor.config().vcs.blame;
    let (view, doc) = current!(editor);
    let text = doc.text();
    let selection = doc.selection(view.id);
    let Some(file) = doc.path() else {
        return;
    };
    if !blame_enabled {
        return;
    }

    let cursor_lin = text.char_to_line(selection.primary().cursor(doc.text().slice(..)));
    let Ok(cursor_line) = TryInto::<u32>::try_into(cursor_lin) else {
        return;
    };

    // gix-blame expects a 1-based line
    let Ok(output) = editor.diff_providers.blame_line(file, cursor_line + 1) else {
        return;
    };

    doc.blame = Some(vec![InlineAnnotation::new(
        text.try_line_to_char(cursor_lin + 1)
            .unwrap_or(text.len_chars())
        // to get the last position in the current line
        - 1,
        output.to_string(),
    )]);
    log::error!("{:?}", doc.blame);
}
