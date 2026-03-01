// canvasx-runtime/src/layout/engine.rs
//
// Top-level layout engine — traverses the CXRD node tree and
// computes layout positions for each node using block flow or flexbox.

use crate::cxrd::document::CxrdDocument;
use crate::cxrd::node::{NodeId};
use crate::cxrd::style::{Display, Position};
use crate::cxrd::value::{Dimension, Rect, EdgeInsets};
use crate::layout::types::LayoutConstraints;

/// Perform a full layout pass on a CXRD document.
///
/// After this, every node's `layout.rect` is populated with its
/// absolute pixel position and size.
pub fn compute_layout(doc: &mut CxrdDocument, viewport_width: f32, viewport_height: f32) {
    let constraints = LayoutConstraints::new(viewport_width, viewport_height);

    // Set root node to fill viewport.
    if let Some(root) = doc.nodes.get_mut(doc.root as usize) {
        root.layout.rect = Rect {
            x: 0.0,
            y: 0.0,
            width: viewport_width,
            height: viewport_height,
        };
        root.layout.content_rect = root.layout.rect;
    }

    // Layout tree recursively (iterative via stack to avoid deep recursion).
    let root_id = doc.root;
    layout_node_recursive(doc, root_id, &constraints, None);
}

/// Recursively layout a node and its children.
///
/// We do this in a slightly awkward way because we can't hold mutable
/// references to multiple nodes simultaneously in a Vec.  We collect
/// child info, compute layout, then write results back.
fn layout_node_recursive(
    doc: &mut CxrdDocument,
    node_id: NodeId,
    constraints: &LayoutConstraints,
    clip: Option<Rect>,
) {
    let node = match doc.nodes.get(node_id as usize) {
        Some(n) => n,
        None => return,
    };

    if matches!(node.style.display, Display::None) {
        return;
    }

    let style = node.style.clone();
    let container_rect = node.layout.content_rect;
    let children: Vec<NodeId> = node.children.clone();

    // Set clip for overflow: hidden containers.
    let child_clip = if matches!(style.overflow, crate::cxrd::style::Overflow::Hidden) {
        Some(container_rect)
    } else {
        clip
    };

    if children.is_empty() {
        return;
    }

    // Determine layout mode.
    let is_flex = matches!(style.display, Display::Flex);

    if is_flex {
        // --- Flexbox layout ---
        // We need to collect mutable refs to children.  Since they're in
        // the same Vec, we work with indexes and unsafe-free tricks.
        layout_flex_children(doc, node_id, container_rect, &children, constraints);
    } else {
        // --- Block flow layout ---
        layout_block_children(doc, container_rect, &children, constraints);
    }

    // Handle absolute-positioned children.
    for &child_id in &children {
        if let Some(child) = doc.nodes.get(child_id as usize) {
            if matches!(child.style.position, Position::Absolute | Position::Fixed) {
                let cs = child.style.clone();
                let containing = if matches!(cs.position, Position::Fixed) {
                    Rect::new(0.0, 0.0, constraints.viewport_width, constraints.viewport_height)
                } else {
                    container_rect
                };
                layout_absolute_child(doc, child_id, containing, constraints);
            }
        }

        // Set clip on children.
        if let Some(child) = doc.nodes.get_mut(child_id as usize) {
            child.layout.clip = child_clip;
        }
    }

    // Recurse into children.
    for &child_id in &children {
        layout_node_recursive(doc, child_id, constraints, child_clip);
    }
}

/// Layout children using flexbox.
fn layout_flex_children(
    doc: &mut CxrdDocument,
    parent_id: NodeId,
    container_rect: Rect,
    child_ids: &[NodeId],
    constraints: &LayoutConstraints,
) {
    // We need to temporarily extract children to mutate them together with the parent.
    // Since they're all in the same Vec, we use index-based access.
    // First, initialize child rects.
    for &cid in child_ids {
        if let Some(child) = doc.nodes.get(cid as usize) {
            let cs = &child.style;
            if matches!(cs.position, Position::Absolute | Position::Fixed) {
                continue; // Handled separately.
            }
        }
    }

    // Collect non-absolute child IDs for flex layout.
    let flex_children: Vec<NodeId> = child_ids.iter()
        .copied()
        .filter(|&cid| {
            doc.nodes.get(cid as usize)
                .map(|c| !matches!(c.style.position, Position::Absolute | Position::Fixed))
                .unwrap_or(false)
        })
        .collect();

    if flex_children.is_empty() {
        return;
    }

    // We'll do the flex computation using extracted data then write back.
    // Extract the parent style and children as a separate working set.
    let parent_style = doc.nodes[parent_id as usize].style.clone();

    // Pre-resolve child sizes and flex properties, then use a simplified
    // inline flex algorithm (since we can't pass &mut [&mut CxrdNode]).
    let gap = parent_style.gap;
    let dir = parent_style.flex_direction;
    let is_row = matches!(dir, crate::cxrd::style::FlexDirection::Row | crate::cxrd::style::FlexDirection::RowReverse);
    let main_size = if is_row { container_rect.width } else { container_rect.height };
    let cross_size = if is_row { container_rect.height } else { container_rect.width };

    struct ItemData {
        base_main: f32,
        base_cross: f32,
        flex_grow: f32,
        flex_shrink: f32,
        m_start: f32,
        m_end: f32,
        c_start: f32,
        c_end: f32,
        padding: EdgeInsets,
        border: EdgeInsets,
        align: crate::cxrd::style::AlignItems,
    }

    let mut items: Vec<ItemData> = Vec::with_capacity(flex_children.len());
    for &cid in &flex_children {
        let cs = &doc.nodes[cid as usize].style;
        let resolve = |d: Dimension, parent: f32| -> f32 {
            d.resolve(parent, constraints.viewport_width, constraints.viewport_height, cs.font_size, constraints.root_font_size)
        };

        let margin = EdgeInsets {
            top: resolve(cs.margin.top, container_rect.height),
            right: resolve(cs.margin.right, container_rect.width),
            bottom: resolve(cs.margin.bottom, container_rect.height),
            left: resolve(cs.margin.left, container_rect.width),
        };
        let padding = EdgeInsets {
            top: resolve(cs.padding.top, container_rect.height),
            right: resolve(cs.padding.right, container_rect.width),
            bottom: resolve(cs.padding.bottom, container_rect.height),
            left: resolve(cs.padding.left, container_rect.width),
        };
        let border = cs.border_width;

        let (m_start, m_end, c_start, c_end) = if is_row {
            (margin.left, margin.right, margin.top, margin.bottom)
        } else {
            (margin.top, margin.bottom, margin.left, margin.right)
        };

        let basis = if !cs.flex_basis.is_auto() {
            resolve(cs.flex_basis, main_size)
        } else if is_row && !cs.width.is_auto() {
            resolve(cs.width, container_rect.width)
        } else if !is_row && !cs.height.is_auto() {
            resolve(cs.height, container_rect.height)
        } else {
            0.0
        };

        let cross = if is_row && !cs.height.is_auto() {
            resolve(cs.height, container_rect.height)
        } else if !is_row && !cs.width.is_auto() {
            resolve(cs.width, container_rect.width)
        } else {
            0.0
        };

        let align = if cs.align_self != crate::cxrd::style::AlignSelf::Auto {
            match cs.align_self {
                crate::cxrd::style::AlignSelf::FlexStart => crate::cxrd::style::AlignItems::FlexStart,
                crate::cxrd::style::AlignSelf::FlexEnd => crate::cxrd::style::AlignItems::FlexEnd,
                crate::cxrd::style::AlignSelf::Center => crate::cxrd::style::AlignItems::Center,
                crate::cxrd::style::AlignSelf::Stretch => crate::cxrd::style::AlignItems::Stretch,
                crate::cxrd::style::AlignSelf::Auto => parent_style.align_items,
            }
        } else {
            parent_style.align_items
        };

        items.push(ItemData {
            base_main: basis,
            base_cross: cross,
            flex_grow: cs.flex_grow,
            flex_shrink: cs.flex_shrink,
            m_start, m_end, c_start, c_end,
            padding, border, align,
        });
    }

    // Flex distribution
    let num_gaps = if items.len() > 1 { (items.len() - 1) as f32 } else { 0.0 };
    let total_gap = gap * num_gaps;
    let total_base: f32 = items.iter().map(|i| i.base_main + i.m_start + i.m_end).sum();
    let free = main_size - total_base - total_gap;
    let total_grow: f32 = items.iter().map(|i| i.flex_grow).sum();
    let total_shrink: f32 = items.iter().map(|i| i.flex_shrink * i.base_main).sum();

    let finals: Vec<f32> = items.iter().map(|item| {
        let mut sz = item.base_main;
        if free > 0.0 && total_grow > 0.0 {
            sz += free * (item.flex_grow / total_grow);
        } else if free < 0.0 && total_shrink > 0.0 {
            sz += free * (item.flex_shrink * item.base_main / total_shrink);
        }
        sz.max(0.0)
    }).collect();

    // Justify-content
    let used: f32 = finals.iter().sum::<f32>()
        + items.iter().map(|i| i.m_start + i.m_end).sum::<f32>()
        + total_gap;
    let remaining = (main_size - used).max(0.0);

    let (mut offset, extra_gap) = match parent_style.justify_content {
        crate::cxrd::style::JustifyContent::FlexStart => (0.0, 0.0),
        crate::cxrd::style::JustifyContent::FlexEnd => (remaining, 0.0),
        crate::cxrd::style::JustifyContent::Center => (remaining / 2.0, 0.0),
        crate::cxrd::style::JustifyContent::SpaceBetween => {
            if items.len() > 1 { (0.0, remaining / (items.len() - 1) as f32) } else { (0.0, 0.0) }
        }
        crate::cxrd::style::JustifyContent::SpaceAround => {
            let sp = remaining / items.len() as f32;
            (sp / 2.0, sp)
        }
        crate::cxrd::style::JustifyContent::SpaceEvenly => {
            let sp = remaining / (items.len() + 1) as f32;
            (sp, sp)
        }
    };

    // Position each child
    for (i, &cid) in flex_children.iter().enumerate() {
        let item = &items[i];
        let m = finals[i];

        let c = if item.base_cross > 0.0 {
            item.base_cross
        } else if matches!(item.align, crate::cxrd::style::AlignItems::Stretch) {
            cross_size - item.c_start - item.c_end
        } else {
            cross_size - item.c_start - item.c_end
        };

        let cross_offset = match item.align {
            crate::cxrd::style::AlignItems::FlexStart => item.c_start,
            crate::cxrd::style::AlignItems::FlexEnd => cross_size - c - item.c_end,
            crate::cxrd::style::AlignItems::Center => (cross_size - c) / 2.0,
            _ => item.c_start,
        };

        offset += item.m_start;

        let (x, y, w, h) = if is_row {
            (container_rect.x + offset, container_rect.y + cross_offset, m, c)
        } else {
            (container_rect.x + cross_offset, container_rect.y + offset, c, m)
        };

        let node = &mut doc.nodes[cid as usize];
        node.layout.rect = Rect { x, y, width: w, height: h };
        node.layout.content_rect = Rect {
            x: x + item.padding.left + item.border.left,
            y: y + item.padding.top + item.border.top,
            width: (w - item.padding.horizontal() - item.border.horizontal()).max(0.0),
            height: (h - item.padding.vertical() - item.border.vertical()).max(0.0),
        };
        node.layout.padding = item.padding;

        offset += m + item.m_end + gap + extra_gap;
    }
}

/// Layout children using simple block flow (stack vertically).
fn layout_block_children(
    doc: &mut CxrdDocument,
    container_rect: Rect,
    child_ids: &[NodeId],
    constraints: &LayoutConstraints,
) {
    let mut y_cursor = container_rect.y;

    for &cid in child_ids {
        let cs = doc.nodes[cid as usize].style.clone();
        if matches!(cs.display, Display::None) || matches!(cs.position, Position::Absolute | Position::Fixed) {
            continue;
        }

        let resolve = |d: Dimension, parent: f32| -> f32 {
            d.resolve(parent, constraints.viewport_width, constraints.viewport_height, cs.font_size, constraints.root_font_size)
        };

        let margin = EdgeInsets {
            top: resolve(cs.margin.top, container_rect.height),
            right: resolve(cs.margin.right, container_rect.width),
            bottom: resolve(cs.margin.bottom, container_rect.height),
            left: resolve(cs.margin.left, container_rect.width),
        };
        let padding = EdgeInsets {
            top: resolve(cs.padding.top, container_rect.height),
            right: resolve(cs.padding.right, container_rect.width),
            bottom: resolve(cs.padding.bottom, container_rect.height),
            left: resolve(cs.padding.left, container_rect.width),
        };
        let border = cs.border_width;

        let w = if !cs.width.is_auto() {
            resolve(cs.width, container_rect.width)
        } else {
            container_rect.width - margin.horizontal()
        };

        let h = if !cs.height.is_auto() {
            resolve(cs.height, container_rect.height)
        } else {
            // Auto height: inherit parent height for block children that
            // have flow-mode children.  This ensures wrappers like <html>
            // and <body> fill their container instead of collapsing to 0.
            // A more sophisticated engine would sum children's heights;
            // for now this is a good-enough heuristic.
            let num_children = doc.nodes.get(cid as usize)
                .map(|n| n.children.len()).unwrap_or(0);
            if num_children > 0 {
                // Use remaining container height from cursor.
                (container_rect.y + container_rect.height - y_cursor - margin.vertical()).max(0.0)
            } else {
                // Leaf block with auto height: use a text line height.
                let font_size = cs.font_size.max(14.0);
                let line_h = cs.line_height * font_size;
                padding.vertical() + border.vertical() + line_h
            }
        };

        y_cursor += margin.top;

        let node = &mut doc.nodes[cid as usize];
        node.layout.rect = Rect {
            x: container_rect.x + margin.left,
            y: y_cursor,
            width: w,
            height: h,
        };
        node.layout.content_rect = Rect {
            x: container_rect.x + margin.left + padding.left + border.left,
            y: y_cursor + padding.top + border.top,
            width: (w - padding.horizontal() - border.horizontal()).max(0.0),
            height: (h - padding.vertical() - border.vertical()).max(0.0),
        };
        node.layout.padding = padding;
        node.layout.margin = margin;

        y_cursor += h + margin.bottom;
    }
}

/// Layout an absolutely-positioned child within its containing block.
fn layout_absolute_child(
    doc: &mut CxrdDocument,
    child_id: NodeId,
    containing: Rect,
    constraints: &LayoutConstraints,
) {
    let cs = doc.nodes[child_id as usize].style.clone();

    let resolve = |d: Dimension, parent: f32| -> f32 {
        d.resolve(parent, constraints.viewport_width, constraints.viewport_height, cs.font_size, constraints.root_font_size)
    };

    let padding = EdgeInsets {
        top: resolve(cs.padding.top, containing.height),
        right: resolve(cs.padding.right, containing.width),
        bottom: resolve(cs.padding.bottom, containing.height),
        left: resolve(cs.padding.left, containing.width),
    };
    let border = cs.border_width;

    let w = if !cs.width.is_auto() {
        resolve(cs.width, containing.width)
    } else {
        // Compute from left/right
        let l = if !cs.left.is_auto() { resolve(cs.left, containing.width) } else { 0.0 };
        let r = if !cs.right.is_auto() { resolve(cs.right, containing.width) } else { 0.0 };
        (containing.width - l - r).max(0.0)
    };

    let h = if !cs.height.is_auto() {
        resolve(cs.height, containing.height)
    } else {
        let t = if !cs.top.is_auto() { resolve(cs.top, containing.height) } else { 0.0 };
        let b = if !cs.bottom.is_auto() { resolve(cs.bottom, containing.height) } else { 0.0 };
        (containing.height - t - b).max(0.0)
    };

    let x = if !cs.left.is_auto() {
        containing.x + resolve(cs.left, containing.width)
    } else if !cs.right.is_auto() {
        containing.x + containing.width - resolve(cs.right, containing.width) - w
    } else {
        containing.x
    };

    let y = if !cs.top.is_auto() {
        containing.y + resolve(cs.top, containing.height)
    } else if !cs.bottom.is_auto() {
        containing.y + containing.height - resolve(cs.bottom, containing.height) - h
    } else {
        containing.y
    };

    let node = &mut doc.nodes[child_id as usize];
    node.layout.rect = Rect { x, y, width: w, height: h };
    node.layout.content_rect = Rect {
        x: x + padding.left + border.left,
        y: y + padding.top + border.top,
        width: (w - padding.horizontal() - border.horizontal()).max(0.0),
        height: (h - padding.vertical() - border.vertical()).max(0.0),
    };
    node.layout.padding = padding;
}
