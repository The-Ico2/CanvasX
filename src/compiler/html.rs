// canvasx-runtime/src/compiler/html.rs
//
// HTML subset parser for the CanvasX Runtime.
// Converts restricted HTML into CXRD nodes.
//
// Supported elements:
//   div, span, p, h1–h6, img, button, input, label, svg, path, section
//   Custom: <data-bind> for live system data.
//
// Attributes: class, id, style (inline), data-*, src, alt

use crate::cxrd::document::{CxrdDocument, SceneType};
use crate::cxrd::node::{CxrdNode, NodeKind, ImageFit, NodeId, EventBinding, EventAction};
use crate::cxrd::input::{InputKind, TextInputType, ButtonVariant, CheckboxStyle};
use crate::cxrd::style::ComputedStyle;
use crate::compiler::css::{parse_css, apply_property, parse_color, CssRule, SelectorPart};
use std::collections::HashMap;
use std::path::Path;

/// Compile an HTML file + CSS into a CXRD document.
///
/// `html_source` — the HTML content.
/// `css_source` — the CSS content (from <link> or <style>).
/// `asset_dir` — base directory for resolving local asset paths.
/// `scene_type` — what kind of scene this is (wallpaper, widget, etc.).
pub fn compile_html(
    html_source: &str,
    css_source: &str,
    name: &str,
    scene_type: SceneType,
    _asset_dir: Option<&Path>,
) -> anyhow::Result<CxrdDocument> {
    let mut doc = CxrdDocument::new(name, scene_type);

    // 1. Parse CSS rules.
    let rules = parse_css(css_source);

    // 2. Extract CSS custom properties (:root variables).
    let mut variables: HashMap<String, String> = HashMap::new();
    for rule in &rules {
        if rule.selector == ":root" {
            for (prop, val) in &rule.declarations {
                if prop.starts_with("--") {
                    variables.insert(prop.clone(), val.clone());
                }
            }
        }
    }
    doc.variables = variables.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

    // 2b. Extract document background from body/html CSS rules.
    extract_document_background(&mut doc, &rules, &variables);

    // 3. Parse HTML into node tree.
    let tokens = tokenize_html(html_source);
    let (root_children, _) = build_node_tree(&tokens, 0);

    // 4. Add nodes to document and apply CSS.
    for child in root_children {
        let child_id = add_node_recursive(&mut doc, child, &rules, &variables);
        doc.add_child(doc.root, child_id);
    }

    // 5. Apply root styles.
    if let Some(root) = doc.get_node_mut(doc.root) {
        apply_rules_to_node(root, &rules, &variables);
    }

    Ok(doc)
}

/// Extract document background color from body/html/:root CSS rules.
///
/// The sentinel.default wallpaper uses `background: var(--bg-color)` on body,
/// which resolves to a hex color. We check body, html, and :root rules in
/// order, taking the last match (highest specificity).
fn extract_document_background(
    doc: &mut CxrdDocument,
    rules: &[CssRule],
    variables: &HashMap<String, String>,
) {
    use crate::compiler::css::resolve_var_pub;

    let bg_selectors = ["html", "body", ":root", "html,body", "html, body"];
    for rule in rules {
        let sel = rule.selector.trim();
        // Check if selector targets html, body, or :root.
        let matches = bg_selectors.iter().any(|s| sel == *s)
            || sel.split(',').any(|part| {
                let part = part.trim();
                part == "html" || part == "body"
            });

        if !matches {
            continue;
        }

        for (prop, val) in &rule.declarations {
            if prop == "background" || prop == "background-color" {
                let resolved = resolve_var_pub(val, variables);
                if let Some(color) = parse_color(&resolved) {
                    doc.background = color;
                }
            }
        }
    }
}

/// A temporary parsed HTML node before adding to the document.
struct ParsedNode {
    tag: String,
    classes: Vec<String>,
    id: Option<String>,
    attributes: HashMap<String, String>,
    inline_style: String,
    text_content: Option<String>,
    children: Vec<ParsedNode>,
}

/// Tokenize HTML into a flat list of events.
#[derive(Debug)]
enum HtmlToken {
    OpenTag {
        tag: String,
        attributes: HashMap<String, String>,
        self_closing: bool,
    },
    CloseTag {
        tag: String,
    },
    Text(String),
}

fn tokenize_html(source: &str) -> Vec<HtmlToken> {
    let mut tokens = Vec::new();
    let mut pos = 0;
    let bytes = source.as_bytes();

    while pos < bytes.len() {
        if bytes[pos] == b'<' {
            // Check for comment
            if pos + 3 < bytes.len() && &source[pos..pos+4] == "<!--" {
                if let Some(end) = source[pos..].find("-->") {
                    pos += end + 3;
                    continue;
                }
            }

            // Check for close tag
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'/' {
                pos += 2;
                let tag_start = pos;
                while pos < bytes.len() && bytes[pos] != b'>' {
                    pos += 1;
                }
                let tag = source[tag_start..pos].trim().to_lowercase();
                pos += 1; // skip >
                tokens.push(HtmlToken::CloseTag { tag });
                continue;
            }

            // Open tag
            pos += 1; // skip <
            let tag_start = pos;
            while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'>' && bytes[pos] != b'/' {
                pos += 1;
            }
            let tag = source[tag_start..pos].trim().to_lowercase();

            // Parse attributes
            let mut attributes = HashMap::new();
            loop {
                // Skip whitespace
                while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                if pos >= bytes.len() || bytes[pos] == b'>' || bytes[pos] == b'/' {
                    break;
                }

                // Attribute name
                let attr_start = pos;
                while pos < bytes.len() && bytes[pos] != b'=' && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'>' && bytes[pos] != b'/' {
                    pos += 1;
                }
                let attr_name = source[attr_start..pos].to_lowercase();

                if pos < bytes.len() && bytes[pos] == b'=' {
                    pos += 1; // skip =
                    // Attribute value
                    let val = if pos < bytes.len() && (bytes[pos] == b'"' || bytes[pos] == b'\'') {
                        let quote = bytes[pos];
                        pos += 1;
                        let val_start = pos;
                        while pos < bytes.len() && bytes[pos] != quote {
                            pos += 1;
                        }
                        let val = source[val_start..pos].to_string();
                        if pos < bytes.len() { pos += 1; } // skip closing quote
                        val
                    } else {
                        let val_start = pos;
                        while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'>' {
                            pos += 1;
                        }
                        source[val_start..pos].to_string()
                    };
                    attributes.insert(attr_name, val);
                } else {
                    attributes.insert(attr_name, String::new());
                }
            }

            let self_closing = pos < bytes.len() && bytes[pos] == b'/';
            if self_closing { pos += 1; }
            if pos < bytes.len() && bytes[pos] == b'>' { pos += 1; }

            // Skip <script>, <style>, <head>, <meta>, <link> tags entirely
            let skip_tags = ["script", "style", "head", "meta", "link", "title"];
            if skip_tags.contains(&tag.as_str()) {
                if !self_closing {
                    // Find closing tag
                    let close = format!("</{}>", tag);
                    if let Some(end) = source[pos..].to_lowercase().find(&close) {
                        pos += end + close.len();
                    }
                }
                continue;
            }

            tokens.push(HtmlToken::OpenTag { tag, attributes, self_closing });
        } else {
            // Text content
            let text_start = pos;
            while pos < bytes.len() && bytes[pos] != b'<' {
                pos += 1;
            }
            let text = source[text_start..pos].trim();
            if !text.is_empty() {
                tokens.push(HtmlToken::Text(text.to_string()));
            }
        }
    }

    tokens
}

/// Build node tree from tokens.
fn build_node_tree(tokens: &[HtmlToken], start: usize) -> (Vec<ParsedNode>, usize) {
    let mut nodes = Vec::new();
    let mut i = start;

    while i < tokens.len() {
        match &tokens[i] {
            HtmlToken::OpenTag { tag, attributes, self_closing } => {
                let classes: Vec<String> = attributes.get("class")
                    .map(|c| c.split_whitespace().map(String::from).collect())
                    .unwrap_or_default();
                let id = attributes.get("id").cloned();
                let inline_style = attributes.get("style").cloned().unwrap_or_default();

                let mut node = ParsedNode {
                    tag: tag.clone(),
                    classes,
                    id,
                    attributes: attributes.clone(),
                    inline_style,
                    text_content: None,
                    children: Vec::new(),
                };

                if *self_closing || is_void_element(tag) {
                    i += 1;
                } else {
                    let (children, end_pos) = build_node_tree(tokens, i + 1);
                    node.children = children;
                    i = end_pos + 1; // skip past the close tag
                }

                nodes.push(node);
            }
            HtmlToken::CloseTag { .. } => {
                return (nodes, i);
            }
            HtmlToken::Text(text) => {
                nodes.push(ParsedNode {
                    tag: "#text".to_string(),
                    classes: Vec::new(),
                    id: None,
                    attributes: HashMap::new(),
                    inline_style: String::new(),
                    text_content: Some(text.clone()),
                    children: Vec::new(),
                });
                i += 1;
            }
        }
    }

    (nodes, i)
}

fn is_void_element(tag: &str) -> bool {
    matches!(tag, "img" | "br" | "hr" | "input" | "meta" | "link" | "source" | "svg" | "path" | "line" | "circle" | "rect" | "polyline" | "ellipse" | "polygon")
}

/// Add a parsed node tree to the CXRD document.
fn add_node_recursive(
    doc: &mut CxrdDocument,
    parsed: ParsedNode,
    rules: &[CssRule],
    variables: &HashMap<String, String>,
) -> NodeId {
    let kind = determine_node_kind(&parsed);

    // For widget elements, children are consumed by the widget (label, options, etc.)
    // and should not be added as child scene nodes.
    let skip_children = matches!(&kind,
        NodeKind::Input(InputKind::Button { .. }) |
        NodeKind::Input(InputKind::Dropdown { .. }) |
        NodeKind::Input(InputKind::TextArea { .. })
    );

    let mut node = CxrdNode {
        id: 0, // Will be set by add_node
        tag: Some(parsed.tag.clone()),
        classes: parsed.classes.clone(),
        kind,
        style: ComputedStyle::default(),
        children: Vec::new(),
        events: extract_event_bindings(&parsed),
        animations: Vec::new(),
        layout: Default::default(),
    };

    // Apply CSS rules in order.
    apply_rules_to_node(&mut node, rules, variables);

    // Apply inline styles (highest specificity).
    if !parsed.inline_style.is_empty() {
        for decl in parsed.inline_style.split(';') {
            let decl = decl.trim();
            if let Some((prop, val)) = decl.split_once(':') {
                apply_property(&mut node.style, prop.trim(), val.trim(), variables);
            }
        }
    }

    let node_id = doc.add_node(node);

    // Add children (unless consumed by widget).
    if !skip_children {
        for child in parsed.children {
            let child_id = add_node_recursive(doc, child, rules, variables);
            doc.add_child(node_id, child_id);
        }
    }

    node_id
}

/// Extract text content from direct #text children of a parsed node.
fn extract_text_content(parsed: &ParsedNode) -> String {
    let mut text = String::new();
    for child in &parsed.children {
        if child.tag == "#text" {
            if let Some(t) = &child.text_content {
                if !text.is_empty() { text.push(' '); }
                text.push_str(t);
            }
        }
    }
    text
}

/// Extract <option> children from a <select> element.
fn extract_select_options(parsed: &ParsedNode) -> Vec<(String, String)> {
    let mut options = Vec::new();
    for child in &parsed.children {
        if child.tag == "option" {
            let value = child.attributes.get("value").cloned().unwrap_or_default();
            let label = extract_text_content(child);
            let label = if label.is_empty() { value.clone() } else { label };
            options.push((value, label));
        }
    }
    options
}

/// Extract event bindings from data-* attributes.
fn extract_event_bindings(parsed: &ParsedNode) -> Vec<EventBinding> {
    let mut events = Vec::new();

    // data-action with optional data-event (defaults to "click")
    if let Some(action_type) = parsed.attributes.get("data-action") {
        let event_type = parsed.attributes.get("data-event")
            .cloned()
            .unwrap_or_else(|| "click".to_string());

        let action = match action_type.as_str() {
            "navigate" => {
                let target = parsed.attributes.get("data-target")
                    .cloned().unwrap_or_default();
                EventAction::Navigate { scene_id: target }
            }
            "ipc" => {
                let ns = parsed.attributes.get("data-ns")
                    .cloned().unwrap_or_default();
                let cmd = parsed.attributes.get("data-cmd")
                    .cloned().unwrap_or_default();
                let args = parsed.attributes.get("data-args")
                    .and_then(|a| serde_json::from_str(a).ok());
                EventAction::IpcCommand { ns, cmd, args }
            }
            "toggle-class" => {
                let class = parsed.attributes.get("data-class")
                    .cloned().unwrap_or_default();
                EventAction::ToggleClass { target: 0, class }
            }
            _ => {
                // Treat the action string as an IPC command name.
                EventAction::IpcCommand {
                    ns: String::new(),
                    cmd: action_type.clone(),
                    args: None,
                }
            }
        };

        events.push(EventBinding { event: event_type, action });
    }

    // data-navigate shorthand
    if let Some(target) = parsed.attributes.get("data-navigate") {
        events.push(EventBinding {
            event: "click".to_string(),
            action: EventAction::Navigate { scene_id: target.clone() },
        });
    }

    events
}

/// Determine the NodeKind from the HTML element.
fn determine_node_kind(parsed: &ParsedNode) -> NodeKind {
    match parsed.tag.as_str() {
        "#text" => {
            NodeKind::Text {
                content: parsed.text_content.clone().unwrap_or_default(),
            }
        }
        "img" => {
            NodeKind::Image {
                asset_index: 0, // Will be resolved during asset bundling.
                fit: ImageFit::Cover,
            }
        }
        "data-bind" => {
            let binding = parsed.attributes.get("data-binding")
                .or_else(|| parsed.attributes.get("binding"))
                .cloned()
                .unwrap_or_default();
            let format = parsed.attributes.get("format").cloned();
            NodeKind::DataBound { binding, format }
        }
        "path" => {
            let d = parsed.attributes.get("d").cloned().unwrap_or_default();
            NodeKind::SvgPath {
                d,
                stroke_color: None,
                fill_color: None,
                stroke_width: 2.0,
            }
        }

        // ── Interactive elements ────────────────────────────────────

        "button" => {
            let label = extract_text_content(parsed);
            let disabled = parsed.attributes.contains_key("disabled");
            let variant = match parsed.attributes.get("data-variant").map(|s| s.as_str()) {
                Some("primary") => ButtonVariant::Primary,
                Some("secondary") => ButtonVariant::Secondary,
                Some("danger") => ButtonVariant::Danger,
                Some("ghost") => ButtonVariant::Ghost,
                Some("link") => ButtonVariant::Link,
                _ => ButtonVariant::Primary,
            };
            NodeKind::Input(InputKind::Button { label, disabled, variant })
        }

        "input" => {
            let input_type = parsed.attributes.get("type")
                .map(|s| s.as_str()).unwrap_or("text");
            match input_type {
                "checkbox" => {
                    let checked = parsed.attributes.contains_key("checked");
                    let disabled = parsed.attributes.contains_key("disabled");
                    let label = parsed.attributes.get("data-label")
                        .cloned().unwrap_or_default();
                    let style = match parsed.attributes.get("data-style").map(|s| s.as_str()) {
                        Some("toggle") => CheckboxStyle::Toggle,
                        _ => CheckboxStyle::Checkbox,
                    };
                    NodeKind::Input(InputKind::Checkbox { label, checked, disabled, style })
                }
                "range" => {
                    let value = parsed.attributes.get("value")
                        .and_then(|v| v.parse().ok()).unwrap_or(50.0);
                    let min = parsed.attributes.get("min")
                        .and_then(|v| v.parse().ok()).unwrap_or(0.0);
                    let max = parsed.attributes.get("max")
                        .and_then(|v| v.parse().ok()).unwrap_or(100.0);
                    let step = parsed.attributes.get("step")
                        .and_then(|v| v.parse().ok()).unwrap_or(1.0);
                    let disabled = parsed.attributes.contains_key("disabled");
                    NodeKind::Input(InputKind::Slider {
                        value, min, max, step, disabled, show_value: true,
                    })
                }
                _ => {
                    // text, password, number, email, search
                    let placeholder = parsed.attributes.get("placeholder")
                        .cloned().unwrap_or_default();
                    let value = parsed.attributes.get("value")
                        .cloned().unwrap_or_default();
                    let max_length = parsed.attributes.get("maxlength")
                        .and_then(|v| v.parse().ok()).unwrap_or(0);
                    let read_only = parsed.attributes.contains_key("readonly");
                    let kind = match input_type {
                        "password" => TextInputType::Password,
                        "number"   => TextInputType::Number,
                        "email"    => TextInputType::Email,
                        "search"   => TextInputType::Search,
                        _          => TextInputType::Text,
                    };
                    NodeKind::Input(InputKind::TextInput {
                        placeholder, value, max_length, read_only, input_type: kind,
                    })
                }
            }
        }

        "select" => {
            let options = extract_select_options(parsed);
            let selected = parsed.attributes.get("value").cloned();
            let placeholder = parsed.attributes.get("placeholder")
                .cloned().unwrap_or_else(|| "Select...".to_string());
            let disabled = parsed.attributes.contains_key("disabled");
            NodeKind::Input(InputKind::Dropdown {
                options, selected, placeholder, disabled, open: false,
            })
        }

        "textarea" => {
            let placeholder = parsed.attributes.get("placeholder")
                .cloned().unwrap_or_default();
            let value = extract_text_content(parsed);
            let max_length = parsed.attributes.get("maxlength")
                .and_then(|v| v.parse().ok()).unwrap_or(0);
            let read_only = parsed.attributes.contains_key("readonly");
            let rows = parsed.attributes.get("rows")
                .and_then(|v| v.parse().ok()).unwrap_or(4);
            NodeKind::Input(InputKind::TextArea {
                placeholder, value, max_length, read_only, rows,
            })
        }

        _ => NodeKind::Container,
    }
}

/// Apply matching CSS rules to a node.
fn apply_rules_to_node(
    node: &mut CxrdNode,
    rules: &[CssRule],
    variables: &HashMap<String, String>,
) {
    for rule in rules {
        if selector_matches(&rule.selector_parts, node) {
            for (prop, val) in &rule.declarations {
                apply_property(&mut node.style, prop, val, variables);
            }
        }
    }
}

/// Check if a selector matches a node (simplified — no parent/ancestor matching).
fn selector_matches(parts: &[SelectorPart], node: &CxrdNode) -> bool {
    // For now, only match the last part (simplified specificity).
    if let Some(last) = parts.last() {
        match last {
            SelectorPart::Universal => true,
            SelectorPart::Tag(tag) => node.tag.as_deref() == Some(tag.as_str()),
            SelectorPart::Class(class) => node.classes.contains(class),
            SelectorPart::Id(id) => node.tag.as_deref() == Some(id.as_str()) || node.classes.contains(id),
        }
    } else {
        false
    }
}
