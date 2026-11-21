//! Server-side tab strip: one segment per mapped toplevel along the top of the output.

use std::cell::RefCell;

use smithay::{
    backend::renderer::{
        element::solid::{SolidColorBuffer, SolidColorRenderElement},
        element::{AsRenderElements, Kind},
        Renderer,
    },
    desktop::Space,
    output::Output,
    utils::{Logical, Physical, Point, Rectangle, Scale},
};

use super::WindowElement;

/// Per-output tab strip state.
pub type OutputTabStrip = RefCell<TabStrip>;

pub const TAB_STRIP_HEIGHT: i32 = 28;

const BG: [f32; 4] = [0.12, 0.12, 0.14, 0.92];
const TAB: [f32; 4] = [0.22, 0.22, 0.26, 0.95];
const TAB_ACTIVE: [f32; 4] = [0.0, 0.47, 0.83, 0.95];

#[derive(Debug)]
pub struct TabStrip {
    background: SolidColorBuffer,
    tabs: Vec<SolidColorBuffer>,
    hits: Vec<(Rectangle<i32, Logical>, usize)>,
    width: i32,
}

impl Default for TabStrip {
    fn default() -> Self {
        Self {
            background: SolidColorBuffer::default(),
            tabs: Vec::new(),
            hits: Vec::new(),
            width: 0,
        }
    }
}

impl TabStrip {
    /// Rebuild buffers for the windows currently mapped on `output_geo`.
    pub fn sync(
        &mut self,
        space: &Space<WindowElement>,
        output_geo: Rectangle<i32, Logical>,
        focus: Option<&WindowElement>,
    ) {
        let width = output_geo.size.w.max(1);
        let windows: Vec<_> = space.elements().cloned().collect();
        let count = windows.len().max(1);
        let tab_w = (width / count as i32).max(48).min(220);
        self.width = width;
        self.background
            .update((width, TAB_STRIP_HEIGHT), BG);
        self.tabs.clear();
        self.hits.clear();
        for (index, window) in windows.iter().enumerate() {
            let x = index as i32 * tab_w;
            if x >= width {
                break;
            }
            let w = tab_w.min(width - x).max(1);
            let active = focus.is_some_and(|focus| focus == window);
            let mut buf = SolidColorBuffer::default();
            buf.update((w, TAB_STRIP_HEIGHT - 4), if active { TAB_ACTIVE } else { TAB });
            self.tabs.push(buf);
            self.hits.push((
                Rectangle::new(
                    (output_geo.loc.x + x, output_geo.loc.y + 2).into(),
                    (w, TAB_STRIP_HEIGHT - 4).into(),
                ),
                index,
            ));
        }
    }

    /// Absolute logical hit test → window index in `space.elements()` order.
    pub fn hit(&self, absolute: Point<f64, Logical>) -> Option<usize> {
        let point = Point::from((absolute.x as i32, absolute.y as i32));
        self.hits
            .iter()
            .find(|(rect, _)| rect.contains(point))
            .map(|(_, index)| *index)
    }
}

pub fn ensure_tab_strip(output: &Output) {
    output
        .user_data()
        .insert_if_missing(OutputTabStrip::default);
}

impl<R: Renderer> AsRenderElements<R> for TabStrip {
    type RenderElement = SolidColorRenderElement;

    fn render_elements<C: From<Self::RenderElement>>(
        &self,
        _renderer: &mut R,
        location: Point<i32, Physical>,
        scale: Scale<f64>,
        alpha: f32,
    ) -> Vec<C> {
        let mut out = Vec::with_capacity(self.tabs.len() + 1);
        out.push(
            SolidColorRenderElement::from_buffer(
                &self.background,
                location,
                scale,
                alpha,
                Kind::Unspecified,
            )
            .into(),
        );
        let tab_w = if self.tabs.is_empty() {
            0
        } else {
            (self.width / self.tabs.len() as i32).max(48).min(220)
        };
        for (index, tab) in self.tabs.iter().enumerate() {
            let x = ((index as i32 * tab_w) as f64 * scale.x) as i32;
            let y = (2.0 * scale.y) as i32;
            out.push(
                SolidColorRenderElement::from_buffer(
                    tab,
                    location + Point::from((x, y)),
                    scale,
                    alpha,
                    Kind::Unspecified,
                )
                .into(),
            );
        }
        out
    }
}
