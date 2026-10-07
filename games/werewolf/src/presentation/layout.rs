//! Board-first geometry shared by portraits, hit testing and fixed action slots (doc 04 §10).
use super::{index_f32, rect, Rect, Vec2, Viewport};

#[derive(Clone, Copy, Debug)]
pub(super) struct Layout {
    pub viewport: Vec2,
    pub compact: bool,
    pub landscape: bool,
    pub content: Rect,
    pub card: Rect,
    pub table: Rect,
    pub reveal: Rect,
    pub dock: Rect,
    pub footer: Rect,
    pub dialog: Rect,
}

impl Layout {
    /// Below these bounds, render a public resize message instead of overlapping private UI.
    pub fn supports(viewport: Viewport) -> bool {
        let size = viewport.size();
        size.x >= 280.0 && (size.y >= 500.0 || size.x >= 600.0 && size.y >= 300.0)
    }

    pub fn new(viewport: Viewport) -> Self {
        let size = viewport.size();
        let compact = size.x < 760.0 || size.y < 620.0;
        let landscape = size.x >= 600.0 && size.y < 620.0;
        let margin = 12.0_f32.min(size.x * 0.03);
        let header = if size.y < 500.0 { 66.0 } else { 100.0 };
        let footer = rect(
            margin,
            (size.y - 52.0).max(header),
            size.x - margin * 2.0,
            44.0,
        );
        let (table, reveal, dock, card) = if compact && !landscape {
            let dock = rect(
                margin,
                footer.origin().y - 112.0,
                size.x - margin * 2.0,
                104.0,
            );
            let reveal = rect(margin, dock.origin().y - 52.0, dock.size().x, 44.0);
            let table = rect(
                margin,
                header,
                dock.size().x,
                (reveal.origin().y - header - 8.0).max(156.0),
            );
            let cw = (size.x - 64.0)
                .min((size.y - 330.0).max(180.0) / 1.5)
                .min(256.0);
            let card = rect((size.x - cw) * 0.5, 64.0, cw, cw * 1.5);
            (table, reveal, dock, card)
        } else if landscape {
            let sidebar = (size.x * 0.30).clamp(180.0, 260.0);
            let table = rect(
                margin,
                header,
                size.x - sidebar - margin * 3.0,
                footer.origin().y - header - 8.0,
            );
            let x = table.origin().x + table.size().x + margin;
            let reveal = rect(x, header, sidebar, 44.0);
            let dock = rect(x, header + 52.0, sidebar, 104.0);
            let cw = (size.y - 100.0).clamp(120.0, 240.0) / 1.5;
            let card = rect(24.0, 54.0, cw, cw * 1.5);
            (table, reveal, dock, card)
        } else {
            let sidebar = (size.x * 0.23).clamp(220.0, 300.0);
            let dock = rect(
                margin,
                footer.origin().y - 112.0,
                size.x - margin * 2.0,
                104.0,
            );
            let table = rect(
                margin,
                header,
                size.x - sidebar - margin * 3.0,
                dock.origin().y - header - 12.0,
            );
            let cw = sidebar
                .min((table.size().y - 100.0).max(120.0) / 1.5)
                .min(230.0);
            let x = table.origin().x + table.size().x + margin + (sidebar - cw) * 0.5;
            let card = rect(x, header + 34.0, cw, cw * 1.5);
            let reveal = rect(x, card.origin().y + card.size().y + 8.0, cw, 44.0);
            (table, reveal, dock, card)
        };
        let content = rect(
            margin,
            header,
            size.x - margin * 2.0,
            footer.origin().y - header,
        );
        let dialog = rect(
            margin,
            (size.y * 0.16).max(8.0),
            size.x - margin * 2.0,
            (size.y * 0.68).max(244.0),
        );
        Self {
            viewport: size,
            compact,
            landscape,
            content,
            card,
            table,
            reveal,
            dock,
            footer,
            dialog,
        }
    }

    /// Bounded options geometry shared by the modal and its real input targets.
    pub fn options(self) -> OptionsLayout {
        let condensed = self.viewport.y < 420.0;
        let width = (self.viewport.x - 32.0).min(520.0);
        let height = if condensed { 276.0 } else { 408.0 };
        let dialog = rect(
            (self.viewport.x - width) * 0.5,
            (self.viewport.y - height) * 0.5,
            width,
            height,
        );
        OptionsLayout {
            dialog,
            motion: rect(
                dialog.origin().x + 16.0,
                dialog.origin().y + if condensed { 48.0 } else { 88.0 },
                width - 32.0,
                if condensed { 52.0 } else { 88.0 },
            ),
            tools_y: dialog.origin().y + if condensed { 128.0 } else { 240.0 },
            row_step: if condensed { 48.0 } else { 52.0 },
            condensed,
        }
    }

    /// A 4×3 mobile board, or bounded desktop ellipse; 12 seats remain visible together.
    pub fn seat_rect(self, index: usize, count: usize) -> Rect {
        let table = self.table;
        if self.compact {
            let cols = 4;
            let rows = count.div_ceil(cols).max(1);
            let top = if self.landscape {
                24.0
            } else if table.size().y < 330.0 {
                36.0
            } else {
                86.0
            };
            let cell_w = (table.size().x - 16.0) / 4.0;
            let cell_h = ((table.size().y - top - 8.0) / index_f32(rows)).max(44.0);
            rect(
                table.origin().x + 8.0 + cell_w * index_f32(index % cols),
                table.origin().y + top + cell_h * index_f32(index / cols),
                cell_w,
                cell_h,
            )
        } else {
            let angle = -core::f32::consts::FRAC_PI_2
                + index_f32(index) * core::f32::consts::TAU / index_f32(count.max(1));
            let center = table.origin() + table.size() * 0.5;
            let w = (table.size().x / 7.0).clamp(62.0, 90.0);
            let h = ((table.size().y - 66.0) / 5.0).clamp(44.0, 100.0);
            let radius = Vec2::new(
                (table.size().x - w - 40.0) * 0.5,
                (table.size().y - h - 66.0) * 0.5,
            );
            rect(
                center.x + radius.x * angle.cos() - w * 0.5,
                center.y + radius.y * angle.sin() - h * 0.5,
                w,
                h,
            )
        }
    }
}

/// Game-owned options grouping, independent of private-card drawer geometry.
pub(super) struct OptionsLayout {
    pub dialog: Rect,
    pub motion: Rect,
    pub tools_y: f32,
    pub row_step: f32,
    pub condensed: bool,
}
