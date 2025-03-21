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
        if self.cursor != pos.doc_line {
            return Position::new(0, 0);
        }
        let row = pos.visual_line;
        let col = virt_off.col as u16;
        let width = renderer.viewport.width;
        let start_col = col - renderer.offset.col as u16;
        // start drawing the git blame 6 spaces after the end of the line
        let draw_col = col + 6;

        let end_col = renderer
            .column_in_bounds(draw_col as usize, 1)
            .then(|| {
                renderer
                    .set_string_truncated(
                        renderer.viewport.x + draw_col,
                        row,
                        &self.message,
                        width.saturating_sub(draw_col) as usize,
                        |_| self.style,
                        true,
                        false,
                    )
                    .0
            })
            .unwrap_or(start_col);

        let col_off = end_col - start_col;

        Position::new(0, col_off as usize)
    }
}
