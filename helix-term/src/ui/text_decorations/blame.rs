#![allow(dead_code, unused_variables, unused_mut)]

use helix_core::doc_formatter::FormattedGrapheme;
use helix_core::Position;

use helix_view::theme::Style;
use helix_view::{Document, Theme};

use crate::ui::document::{LinePos, TextRenderer};
use crate::ui::text_decorations::Decoration;

pub struct EolBlame<'a> {
    message: String,
    doc: &'a Document,
    cursor: usize,
    style: Style,
}

impl<'a> EolBlame<'a> {
    pub fn new(doc: &'a Document, theme: &Theme, cursor: usize, message: String) -> Self {
        EolBlame {
            style: theme.get("ui.virtual.blame"),
            message,
            doc,
            cursor,
        }
    }
}

impl Decoration for EolBlame<'_> {
    // fn decorate_line(&mut self, renderer: &mut TextRenderer, pos: LinePos) {
    //     // renderer.draw_dec
    //     //     ration_grapheme(grapheme, style, row, col)
    //     let col_off = 50;
    // }

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
        // if col != self.cursor as u16 {
        //     return Position::new(0, 0);
        // }
        let style = self.style;
        let width = renderer.viewport.width;
        let start_col = col - renderer.offset.col as u16;
        // start drawing the git blame 1 space after the end of the line
        let draw_col = col + 1;

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

    // fn reset_pos(&mut self, _pos: usize) -> usize {
    //     usize::MAX
    // }

    // fn skip_concealed_anchor(&mut self, conceal_end_char_idx: usize) -> usize {
    //     self.reset_pos(conceal_end_char_idx)
    // }

    // fn decorate_grapheme(
    //     &mut self,
    //     _renderer: &mut TextRenderer,
    //     _grapheme: &FormattedGrapheme,
    // ) -> usize {
    //     usize::MAX
    // }
}
