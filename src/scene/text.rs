// canvasx-runtime/src/scene/text.rs
//
// Text painting — extracts all text nodes from the CXRD tree and builds
// glyphon TextArea entries for the text renderer.

use crate::cxrd::document::CxrdDocument;
use crate::cxrd::node::{NodeId, NodeKind};
use crate::cxrd::style::{Display};
use glyphon::{Attrs, Buffer, Color as GlyphonColor, Family, Metrics, Shaping, TextArea, TextBounds, Weight};
use std::collections::HashMap;

/// Holds prepared text buffers for all text nodes in a document.
pub struct TextPainter {
    /// Prepared text buffers keyed by node ID.
    pub buffers: HashMap<u32, Buffer>,
}

impl TextPainter {
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }

    /// Prepare text buffers for all text nodes in the document.
    /// Call this after layout and before rendering.
    pub fn prepare(
        &mut self,
        doc: &CxrdDocument,
        font_system: &mut glyphon::FontSystem,
        data_values: &HashMap<String, String>,
    ) {
        self.buffers.clear();
        self.prepare_node(doc, doc.root, font_system, data_values);
    }

    fn prepare_node(
        &mut self,
        doc: &CxrdDocument,
        node_id: NodeId,
        font_system: &mut glyphon::FontSystem,
        data_values: &HashMap<String, String>,
    ) {
        let node = match doc.get_node(node_id) {
            Some(n) => n,
            None => return,
        };

        if matches!(node.style.display, Display::None) {
            return;
        }

        let text_content = match &node.kind {
            NodeKind::Text { content } => Some(content.clone()),
            NodeKind::DataBound { binding, format } => {
                let raw = data_values.get(binding).cloned().unwrap_or_default();
                Some(if let Some(fmt) = format {
                    fmt.replace("{}", &raw)
                } else {
                    raw
                })
            }
            _ => None,
        };

        if let Some(content) = text_content {
            let style = &node.style;
            let rect = &node.layout.content_rect;

            if rect.width > 0.0 && !content.is_empty() {
                let font_size = style.font_size;
                let line_height = style.line_height * font_size;
                let metrics = Metrics::new(font_size, line_height);

                let mut buffer = Buffer::new(font_system, metrics);

                let family = if style.font_family.is_empty() {
                    Family::SansSerif
                } else {
                    Family::Name(&style.font_family)
                };

                let weight = Weight(style.font_weight.0);

                let attrs = Attrs::new()
                    .family(family)
                    .weight(weight);

                buffer.set_size(font_system, Some(rect.width), Some(rect.height));
                buffer.set_text(font_system, &content, &attrs, Shaping::Advanced, None);
                buffer.shape_until_scroll(font_system, false);

                self.buffers.insert(node.id, buffer);
            }
        }

        for &child_id in &node.children {
            self.prepare_node(doc, child_id, font_system, data_values);
        }
    }

    /// Build TextArea references for the renderer.
    /// The returned Vec borrows from `self.buffers`, so `self` must outlive the render call.
    pub fn text_areas<'a>(&'a self, doc: &'a CxrdDocument) -> Vec<TextArea<'a>> {
        let mut areas = Vec::new();

        for (node_id, buffer) in &self.buffers {
            let node = match doc.get_node(*node_id) {
                Some(n) => n,
                None => continue,
            };

            let rect = &node.layout.content_rect;
            let color = &node.style.color;

            areas.push(TextArea {
                buffer,
                left: rect.x,
                top: rect.y,
                scale: 1.0,
                bounds: TextBounds {
                    left: rect.x as i32,
                    top: rect.y as i32,
                    right: (rect.x + rect.width) as i32,
                    bottom: (rect.y + rect.height) as i32,
                },
                default_color: GlyphonColor::rgba(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                    (color.a * 255.0) as u8,
                ),
                custom_glyphs: &[],
            });
        }

        areas
    }
}
