use helix_core::Position;

use helix_view::theme::Style;
use helix_view::Theme;

use crate::ui::document::{LinePos, TextRenderer};
use crate::ui::text_decorations::Decoration;

pub struct InlineBlame {
    message: String,
    cursor: usize,
    style: Style,
}

impl InlineBlame {
    pub fn new(theme: &Theme, cursor: usize, message: String) -> Self {
        InlineBlame {
            style: theme.get("ui.virtual.inline-blame"),
            message,
            cursor,
        }
    }
}

impl Decoration for InlineBlame {
    fn render_virt_lines(
        &mut self,
        renderer: &mut TextRenderer,
        pos: LinePos,
        virt_off: Position,
    ) -> Position {
        // do not draw inline blame for lines other than the cursor line
        if self.cursor != pos.doc_line {
            return Position::new(0, 0);
        }

        // where the line in the document ends
        let end_of_line = virt_off.col as u16;
        // length of line in the document
        // draw the git blame 6 spaces after the end of the line
        let start_drawing_at = end_of_line + 6;

        let amount_of_characters_drawn = renderer
            .column_in_bounds(start_drawing_at as usize, 1)
            .then(|| {
                // the column where we stop drawing the blame
                let stopped_drawing_at = renderer
                    .set_string_truncated(
                        renderer.viewport.x + start_drawing_at,
                        pos.visual_line,
                        &self.message,
                        renderer.viewport.width.saturating_sub(start_drawing_at) as usize,
                        |_| self.style,
                        true,
                        false,
                    )
                    .0;

                let line_length = end_of_line - renderer.offset.col as u16;

                stopped_drawing_at - line_length
            })
            .unwrap_or_default();

        Position::new(0, amount_of_characters_drawn as usize)
    }
}
