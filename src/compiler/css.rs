// canvasx-runtime/src/compiler/css.rs
//
// CSS subset parser for the CanvasX Runtime.
// Parses a restricted set of CSS properties into ComputedStyle values.
//
// Supported selectors: tag, .class, #id, descendant combinator.
// Supported properties: see the match arms in `apply_property`.

use crate::cxrd::style::*;
use crate::cxrd::value::{Color, Dimension};
use std::collections::HashMap;

/// A parsed CSS rule.
#[derive(Debug, Clone)]
pub struct CssRule {
    /// Selector string.
    pub selector: String,
    /// Parsed selector components.
    pub selector_parts: Vec<SelectorPart>,
    /// Property declarations.
    pub declarations: Vec<(String, String)>,
}

/// A part of a CSS selector.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectorPart {
    Tag(String),
    Class(String),
    Id(String),
    Universal,
}

/// Parse CSS source into a list of rules.
pub fn parse_css(source: &str) -> Vec<CssRule> {
    let mut rules = Vec::new();
    let mut pos = 0;
    let bytes = source.as_bytes();

    // Strip comments
    let source = strip_comments(source);
    let bytes = source.as_bytes();

    while pos < bytes.len() {
        // Skip whitespace
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }

        // Skip @rules (we don't support most of them)
        if bytes[pos] == b'@' {
            // Find the matching { } block or semicolon
            if let Some(block_start) = source[pos..].find('{') {
                let abs_start = pos + block_start;
                if let Some(block_end) = find_matching_brace(&source, abs_start) {
                    // Check for @keyframes — we handle those
                    let at_rule = &source[pos..abs_start].trim();
                    if at_rule.starts_with("@keyframes") {
                        // TODO: Parse keyframes into AnimationDef
                    }
                    pos = block_end + 1;
                    continue;
                }
            }
            // Skip to next semicolon
            if let Some(semi) = source[pos..].find(';') {
                pos += semi + 1;
            } else {
                break;
            }
            continue;
        }

        // Parse selector
        let selector_start = pos;
        while pos < bytes.len() && bytes[pos] != b'{' {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }
        let selector = source[selector_start..pos].trim().to_string();
        pos += 1; // skip '{'

        // Parse declarations until '}'
        let decl_start = pos;
        let mut depth = 1;
        while pos < bytes.len() && depth > 0 {
            if bytes[pos] == b'{' { depth += 1; }
            if bytes[pos] == b'}' { depth -= 1; }
            if depth > 0 { pos += 1; }
        }
        let decl_block = &source[decl_start..pos];
        pos += 1; // skip '}'

        let declarations = parse_declarations(decl_block);

        // Handle comma-separated selectors: "html, body" → two rules.
        let selector_group: Vec<&str> = selector.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        for sel in selector_group {
            let selector_parts = parse_selector(sel);
            rules.push(CssRule {
                selector: sel.to_string(),
                selector_parts,
                declarations: declarations.clone(),
            });
        }
    }

    rules
}

/// Parse a CSS selector into parts.
fn parse_selector(selector: &str) -> Vec<SelectorPart> {
    let mut parts = Vec::new();
    for token in selector.split_whitespace() {
        if token == "*" {
            parts.push(SelectorPart::Universal);
        } else if let Some(class) = token.strip_prefix('.') {
            parts.push(SelectorPart::Class(class.to_string()));
        } else if let Some(id) = token.strip_prefix('#') {
            parts.push(SelectorPart::Id(id.to_string()));
        } else {
            parts.push(SelectorPart::Tag(token.to_lowercase()));
        }
    }
    parts
}

/// Parse declaration block into (property, value) pairs.
fn parse_declarations(block: &str) -> Vec<(String, String)> {
    let mut decls = Vec::new();
    for decl in block.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        if let Some((prop, val)) = decl.split_once(':') {
            decls.push((prop.trim().to_lowercase(), val.trim().to_string()));
        }
    }
    decls
}

/// Apply a CSS property value to a ComputedStyle.
pub fn apply_property(style: &mut ComputedStyle, property: &str, value: &str, variables: &HashMap<String, String>) {
    // Resolve CSS variables.
    let value = resolve_var(value, variables);
    let value = value.trim();

    match property {
        // --- Display ---
        "display" => {
            style.display = match value {
                "flex" => Display::Flex,
                "block" => Display::Block,
                "inline-block" => Display::InlineBlock,
                "inline" => Display::InlineBlock, // approximate
                "grid" | "inline-grid" => {
                    // Grid → flex fallback.  Use Column direction so children
                    // stack vertically and get the full cross-axis width, which
                    // is the closest approximation when we can't parse
                    // grid-template-columns/rows.
                    style.flex_direction = FlexDirection::Column;
                    Display::Flex
                }
                "none" => Display::None,
                _ => style.display,
            };
        }

        // --- Position ---
        "position" => {
            style.position = match value {
                "relative" => Position::Relative,
                "absolute" => Position::Absolute,
                "fixed" => Position::Fixed,
                _ => style.position,
            };
        }

        // --- Overflow ---
        "overflow" => {
            style.overflow = match value {
                "visible" => Overflow::Visible,
                "hidden" => Overflow::Hidden,
                "scroll" => Overflow::Scroll,
                _ => style.overflow,
            };
        }

        // --- Dimensions ---
        "width" => { style.width = parse_dimension(value); }
        "height" => { style.height = parse_dimension(value); }
        "min-width" => { style.min_width = parse_dimension(value); }
        "min-height" => { style.min_height = parse_dimension(value); }
        "max-width" => { style.max_width = parse_dimension(value); }
        "max-height" => { style.max_height = parse_dimension(value); }

        // --- Margin ---
        "margin" => {
            let parts = parse_shorthand_4(value);
            style.margin.top = parts.0;
            style.margin.right = parts.1;
            style.margin.bottom = parts.2;
            style.margin.left = parts.3;
        }
        "margin-top" => { style.margin.top = parse_dimension(value); }
        "margin-right" => { style.margin.right = parse_dimension(value); }
        "margin-bottom" => { style.margin.bottom = parse_dimension(value); }
        "margin-left" => { style.margin.left = parse_dimension(value); }

        // --- Padding ---
        "padding" => {
            let parts = parse_shorthand_4(value);
            style.padding.top = parts.0;
            style.padding.right = parts.1;
            style.padding.bottom = parts.2;
            style.padding.left = parts.3;
        }
        "padding-top" => { style.padding.top = parse_dimension(value); }
        "padding-right" => { style.padding.right = parse_dimension(value); }
        "padding-bottom" => { style.padding.bottom = parse_dimension(value); }
        "padding-left" => { style.padding.left = parse_dimension(value); }

        // --- Flex ---
        "flex-direction" => {
            style.flex_direction = match value {
                "row" => FlexDirection::Row,
                "row-reverse" => FlexDirection::RowReverse,
                "column" => FlexDirection::Column,
                "column-reverse" => FlexDirection::ColumnReverse,
                _ => style.flex_direction,
            };
        }
        "flex-wrap" => {
            style.flex_wrap = match value {
                "nowrap" => FlexWrap::NoWrap,
                "wrap" => FlexWrap::Wrap,
                "wrap-reverse" => FlexWrap::WrapReverse,
                _ => style.flex_wrap,
            };
        }
        "justify-content" => {
            style.justify_content = match value {
                "flex-start" | "start" => JustifyContent::FlexStart,
                "flex-end" | "end" => JustifyContent::FlexEnd,
                "center" => JustifyContent::Center,
                "space-between" => JustifyContent::SpaceBetween,
                "space-around" => JustifyContent::SpaceAround,
                "space-evenly" => JustifyContent::SpaceEvenly,
                _ => style.justify_content,
            };
        }
        "align-items" => {
            style.align_items = match value {
                "flex-start" | "start" => AlignItems::FlexStart,
                "flex-end" | "end" => AlignItems::FlexEnd,
                "center" => AlignItems::Center,
                "stretch" => AlignItems::Stretch,
                "baseline" => AlignItems::Baseline,
                _ => style.align_items,
            };
        }
        "align-self" => {
            style.align_self = match value {
                "auto" => AlignSelf::Auto,
                "flex-start" | "start" => AlignSelf::FlexStart,
                "flex-end" | "end" => AlignSelf::FlexEnd,
                "center" => AlignSelf::Center,
                "stretch" => AlignSelf::Stretch,
                _ => style.align_self,
            };
        }
        "flex-grow" => {
            if let Ok(v) = value.parse::<f32>() {
                style.flex_grow = v;
            }
        }
        "flex-shrink" => {
            if let Ok(v) = value.parse::<f32>() {
                style.flex_shrink = v;
            }
        }
        "flex-basis" => { style.flex_basis = parse_dimension(value); }
        "gap" => {
            if let Some(v) = parse_px(value) {
                style.gap = v;
            }
        }

        // --- Position offsets ---
        "inset" => {
            // Shorthand: sets top, right, bottom, left simultaneously.
            let parts: Vec<&str> = value.split_whitespace().collect();
            match parts.len() {
                1 => {
                    let v = parse_dimension(parts[0]);
                    style.top = v; style.right = v; style.bottom = v; style.left = v;
                }
                2 => {
                    let tb = parse_dimension(parts[0]);
                    let lr = parse_dimension(parts[1]);
                    style.top = tb; style.bottom = tb; style.right = lr; style.left = lr;
                }
                4 => {
                    style.top = parse_dimension(parts[0]);
                    style.right = parse_dimension(parts[1]);
                    style.bottom = parse_dimension(parts[2]);
                    style.left = parse_dimension(parts[3]);
                }
                _ => {
                    let v = parse_dimension(value);
                    style.top = v; style.right = v; style.bottom = v; style.left = v;
                }
            }
        }
        "top" => { style.top = parse_dimension(value); }
        "right" => { style.right = parse_dimension(value); }
        "bottom" => { style.bottom = parse_dimension(value); }
        "left" => { style.left = parse_dimension(value); }

        // --- Background ---
        "background-color" | "background" => {
            if let Some(color) = parse_color(value) {
                style.background = Background::Solid(color);
            }
            // TODO: parse linear-gradient(), url()
        }

        // --- Border ---
        "border" => {
            // Shorthand: 1px solid #color
            let parts: Vec<&str> = value.split_whitespace().collect();
            if let Some(width) = parts.first().and_then(|v| parse_px(v)) {
                style.border_width = crate::cxrd::value::EdgeInsets::uniform(width);
            }
            if let Some(color) = parts.last().and_then(|v| parse_color(v)) {
                style.border_color = color;
            }
        }
        "border-color" => {
            if let Some(c) = parse_color(value) {
                style.border_color = c;
            }
        }
        "border-width" => {
            if let Some(w) = parse_px(value) {
                style.border_width = crate::cxrd::value::EdgeInsets::uniform(w);
            }
        }
        "border-radius" => {
            // Shorthand: uniform or per-corner
            let parts: Vec<&str> = value.split_whitespace().collect();
            match parts.len() {
                1 => {
                    if let Some(v) = parse_px(parts[0]) {
                        style.border_radius = crate::cxrd::value::CornerRadii::uniform(v);
                    }
                }
                4 => {
                    let tl = parse_px(parts[0]).unwrap_or(0.0);
                    let tr = parse_px(parts[1]).unwrap_or(0.0);
                    let br = parse_px(parts[2]).unwrap_or(0.0);
                    let bl = parse_px(parts[3]).unwrap_or(0.0);
                    style.border_radius = crate::cxrd::value::CornerRadii { top_left: tl, top_right: tr, bottom_right: br, bottom_left: bl };
                }
                _ => {}
            }
        }

        // --- Typography ---
        "color" => {
            if let Some(c) = parse_color(value) {
                style.color = c;
            }
        }
        "font-family" => {
            let family = value.trim_matches(|c: char| c == '"' || c == '\'');
            style.font_family = family.to_string();
        }
        "font-size" => {
            if let Some(v) = parse_px(value) {
                style.font_size = v;
            }
        }
        "font-weight" => {
            let w = match value {
                "normal" => 400,
                "bold" => 700,
                "lighter" => 300,
                "bolder" => 600,
                _ => value.parse::<u16>().unwrap_or(400),
            };
            style.font_weight = FontWeight(w);
        }
        "line-height" => {
            if let Ok(v) = value.parse::<f32>() {
                style.line_height = v;
            } else if let Some(v) = parse_px(value) {
                style.line_height = v / style.font_size.max(1.0);
            }
        }
        "text-align" => {
            style.text_align = match value {
                "left" => TextAlign::Left,
                "center" => TextAlign::Center,
                "right" => TextAlign::Right,
                _ => style.text_align,
            };
        }
        "letter-spacing" => {
            if let Some(v) = parse_px(value) {
                style.letter_spacing = v;
            }
        }

        // --- Visual ---
        "opacity" => {
            if let Ok(v) = value.parse::<f32>() {
                style.opacity = v.clamp(0.0, 1.0);
            }
        }
        "z-index" => {
            if let Ok(v) = value.parse::<i32>() {
                style.z_index = v;
            }
        }

        // --- Box shadow ---
        "box-shadow" => {
            if value == "none" {
                style.box_shadow.clear();
            }
            // TODO: parse box-shadow shorthand
        }

        // --- Transition ---
        "transition" => {
            // TODO: parse transition shorthand into TransitionDef
        }

        _ => {
            // Unsupported property — silently ignore.
            log::debug!("Unsupported CSS property: {}", property);
        }
    }
}

/// Parse a CSS dimension value.
pub fn parse_dimension(value: &str) -> Dimension {
    let value = value.trim();
    if value == "auto" {
        return Dimension::Auto;
    }

    // Handle calc() expressions.
    if value.starts_with("calc(") {
        if let Some(inner) = value.strip_prefix("calc(").and_then(|s| s.strip_suffix(')')) {
            return parse_calc_dimension(inner);
        }
    }

    if let Some(v) = value.strip_suffix("px") {
        if let Ok(n) = v.trim().parse::<f32>() {
            return Dimension::Px(n);
        }
    }
    if let Some(v) = value.strip_suffix('%') {
        if let Ok(n) = v.trim().parse::<f32>() {
            return Dimension::Percent(n);
        }
    }
    if let Some(v) = value.strip_suffix("rem") {
        if let Ok(n) = v.trim().parse::<f32>() {
            return Dimension::Rem(n);
        }
    }
    if let Some(v) = value.strip_suffix("em") {
        if let Ok(n) = v.trim().parse::<f32>() {
            return Dimension::Em(n);
        }
    }
    if let Some(v) = value.strip_suffix("vw") {
        if let Ok(n) = v.trim().parse::<f32>() {
            return Dimension::Vw(n);
        }
    }
    if let Some(v) = value.strip_suffix("vh") {
        if let Ok(n) = v.trim().parse::<f32>() {
            return Dimension::Vh(n);
        }
    }
    // Bare number → px
    if let Ok(n) = value.parse::<f32>() {
        return Dimension::Px(n);
    }
    Dimension::Auto
}

/// Parse a `calc()` expression into a Dimension.
///
/// Handles common patterns:
///   - `calc(100% / N)` → Percent(100/N)
///   - `calc(100% - Npx)` → Percent(100) (approximate — drops px term)
///   - `calc(Npx + Mpx)` → Px(N+M)
///   - `calc(Npx * N)` → Px(result)
///
/// Falls back to evaluating as a pure numeric expression when possible.
fn parse_calc_dimension(expr: &str) -> Dimension {
    let expr = expr.trim();

    // Try to detect the "dominant" unit in the expression.
    // Common pattern: "100% / 1" or "100% / var" (already resolved).
    if expr.contains('%') {
        // Extract the percentage value and any arithmetic after it.
        // Pattern: `N% OP M`
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if let Some(pct_pos) = parts.iter().position(|p| p.ends_with('%')) {
            let pct_val = parts[pct_pos].trim_end_matches('%').parse::<f32>().unwrap_or(100.0);

            // Check for operator + operand after the percentage.
            if pct_pos + 2 < parts.len() {
                let op = parts[pct_pos + 1];
                let rhs_str = parts[pct_pos + 2].trim_end_matches("px");
                let rhs = eval_calc_expr(rhs_str).unwrap_or(1.0);
                match op {
                    "/" => return Dimension::Percent(pct_val / rhs),
                    "*" => return Dimension::Percent(pct_val * rhs),
                    "+" | "-" => {
                        // Mixed units — can't perfectly represent, use percentage.
                        return Dimension::Percent(pct_val);
                    }
                    _ => {}
                }
            }
            return Dimension::Percent(pct_val);
        }
    }

    // Pure px or unitless arithmetic: "10px + 20px", "300 - 50", etc.
    // Strip all "px" suffixes and evaluate as arithmetic.
    let cleaned = expr.replace("px", "");
    if let Some(result) = eval_calc_expr(&cleaned) {
        return Dimension::Px(result);
    }

    Dimension::Auto
}

/// Evaluate a simple arithmetic expression (supports +, -, *, /).
/// Handles operator precedence: * and / before + and -.
fn eval_calc_expr(expr: &str) -> Option<f32> {
    let expr = expr.trim();

    // Tokenize into numbers and operators.
    let mut tokens: Vec<CalcToken> = Vec::new();
    let mut pos = 0;
    let bytes = expr.as_bytes();

    while pos < bytes.len() {
        // Skip whitespace.
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() { break; }

        // Operator?
        if matches!(bytes[pos], b'+' | b'*' | b'/') {
            tokens.push(CalcToken::Op(bytes[pos] as char));
            pos += 1;
            continue;
        }

        // Minus: could be operator or negative sign.
        if bytes[pos] == b'-' {
            // It's a negative sign if: first token, or previous token is an operator.
            let is_neg = tokens.is_empty() || matches!(tokens.last(), Some(CalcToken::Op(_)));
            if !is_neg {
                tokens.push(CalcToken::Op('-'));
                pos += 1;
                continue;
            }
        }

        // Number (possibly negative).
        let num_start = pos;
        if pos < bytes.len() && bytes[pos] == b'-' { pos += 1; }
        while pos < bytes.len() && (bytes[pos].is_ascii_digit() || bytes[pos] == b'.') {
            pos += 1;
        }
        if pos > num_start {
            if let Ok(n) = expr[num_start..pos].parse::<f32>() {
                tokens.push(CalcToken::Num(n));
                continue;
            }
        }

        // Unknown character — skip.
        pos += 1;
    }

    // Evaluate with precedence: first pass handles * and /.
    let mut simplified: Vec<CalcToken> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if let CalcToken::Op(op) = &tokens[i] {
            if (*op == '*' || *op == '/') && !simplified.is_empty() {
                if let (Some(CalcToken::Num(lhs)), Some(CalcToken::Num(rhs))) =
                    (simplified.last().cloned(), tokens.get(i + 1))
                {
                    let result = if *op == '*' { lhs * rhs } else if rhs.abs() > f32::EPSILON { lhs / rhs } else { lhs };
                    *simplified.last_mut().unwrap() = CalcToken::Num(result);
                    i += 2;
                    continue;
                }
            }
        }
        simplified.push(tokens[i].clone());
        i += 1;
    }

    // Second pass: + and -.
    let mut result = match simplified.first() {
        Some(CalcToken::Num(n)) => *n,
        _ => return None,
    };
    let mut j = 1;
    while j + 1 < simplified.len() {
        if let (CalcToken::Op(op), CalcToken::Num(rhs)) = (&simplified[j], &simplified[j + 1]) {
            match op {
                '+' => result += rhs,
                '-' => result -= rhs,
                _ => {}
            }
            j += 2;
        } else {
            j += 1;
        }
    }

    Some(result)
}

#[derive(Debug, Clone)]
enum CalcToken {
    Num(f32),
    Op(char),
}

/// Parse a px value.
fn parse_px(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(v) = value.strip_suffix("px") {
        v.trim().parse::<f32>().ok()
    } else {
        value.parse::<f32>().ok()
    }
}

/// Parse a CSS color value.
pub fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim();

    // Named colors
    match value {
        "transparent" => return Some(Color::TRANSPARENT),
        "white" => return Some(Color::WHITE),
        "black" => return Some(Color::BLACK),
        "red" => return Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        "green" => return Some(Color::new(0.0, 0.5, 0.0, 1.0)),
        "blue" => return Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        "yellow" => return Some(Color::new(1.0, 1.0, 0.0, 1.0)),
        "orange" => return Some(Color::new(1.0, 0.647, 0.0, 1.0)),
        "gray" | "grey" => return Some(Color::new(0.5, 0.5, 0.5, 1.0)),
        _ => {}
    }

    // Hex
    if value.starts_with('#') {
        return Color::from_hex(value);
    }

    // rgb() / rgba()
    if let Some(args) = value.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
        let nums: Vec<f32> = args.split(',').filter_map(|s| parse_color_component(s)).collect();
        if nums.len() >= 4 {
            let r = if nums[0] > 1.0 { nums[0] / 255.0 } else { nums[0] };
            let g = if nums[1] > 1.0 { nums[1] / 255.0 } else { nums[1] };
            let b = if nums[2] > 1.0 { nums[2] / 255.0 } else { nums[2] };
            return Some(Color::new(r, g, b, nums[3]));
        }
    }

    if let Some(args) = value.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let nums: Vec<f32> = args.split(',').filter_map(|s| parse_color_component(s)).collect();
        if nums.len() >= 3 {
            let r = if nums[0] > 1.0 { nums[0] / 255.0 } else { nums[0] };
            let g = if nums[1] > 1.0 { nums[1] / 255.0 } else { nums[1] };
            let b = if nums[2] > 1.0 { nums[2] / 255.0 } else { nums[2] };
            return Some(Color::new(r, g, b, 1.0));
        }
    }

    None
}

/// Parse a single color component which may be a plain number, percentage,
/// or a `calc()` expression like `calc(50 * 3)`.
fn parse_color_component(s: &str) -> Option<f32> {
    let s = s.trim();
    if s.ends_with('%') {
        return s.trim_end_matches('%').parse::<f32>().ok().map(|v| v / 100.0);
    }
    // Try plain number first.
    if let Ok(v) = s.parse::<f32>() {
        return Some(v);
    }
    // Try calc() expression.
    if let Some(inner) = s.strip_prefix("calc(").and_then(|s| s.strip_suffix(')')) {
        return eval_calc_expr(inner);
    }
    // Try evaluating as arithmetic even without calc() wrapper
    // (handles things like `50 * 3` that result from var() expansion).
    if s.contains('*') || s.contains('/') || (s.contains('+') && !s.starts_with('+')) || (s.contains('-') && !s.starts_with('-') && s.len() > 1) {
        return eval_calc_expr(s);
    }
    None
}

/// Parse a CSS shorthand with 1–4 values (margin, padding, etc.).
fn parse_shorthand_4(value: &str) -> (Dimension, Dimension, Dimension, Dimension) {
    let parts: Vec<&str> = value.split_whitespace().collect();
    match parts.len() {
        1 => {
            let v = parse_dimension(parts[0]);
            (v, v, v, v)
        }
        2 => {
            let tb = parse_dimension(parts[0]);
            let lr = parse_dimension(parts[1]);
            (tb, lr, tb, lr)
        }
        3 => {
            let t = parse_dimension(parts[0]);
            let lr = parse_dimension(parts[1]);
            let b = parse_dimension(parts[2]);
            (t, lr, b, lr)
        }
        4 => {
            (parse_dimension(parts[0]), parse_dimension(parts[1]),
             parse_dimension(parts[2]), parse_dimension(parts[3]))
        }
        _ => (Dimension::Px(0.0), Dimension::Px(0.0), Dimension::Px(0.0), Dimension::Px(0.0)),
    }
}

/// Resolve CSS `var(--name)` references.
fn resolve_var(value: &str, variables: &HashMap<String, String>) -> String {
    resolve_var_pub(value, variables)
}

/// Public version of resolve_var for use by the HTML compiler.
pub fn resolve_var_pub(value: &str, variables: &HashMap<String, String>) -> String {
    let mut result = value.to_string();
    // Simple var() resolution (non-nested).
    while let Some(start) = result.find("var(") {
        if let Some(end) = result[start..].find(')') {
            let inner = &result[start + 4..start + end].trim();
            // Handle default value: var(--name, default)
            let (var_name, default) = if let Some((name, def)) = inner.split_once(',') {
                (name.trim(), Some(def.trim().to_string()))
            } else {
                (*inner, None)
            };

            let replacement = variables.get(var_name)
                .cloned()
                .or(default)
                .unwrap_or_default();

            result = format!("{}{}{}", &result[..start], replacement, &result[start + end + 1..]);
        } else {
            break;
        }
    }
    result
}

/// Strip CSS comments.
fn strip_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut in_comment = false;
    let bytes = source.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if !in_comment && i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            in_comment = true;
            i += 2;
        } else if in_comment && i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
            in_comment = false;
            i += 2;
        } else if !in_comment {
            result.push(bytes[i] as char);
            i += 1;
        } else {
            i += 1;
        }
    }

    result
}

/// Find the matching closing brace for an opening brace at `start`.
fn find_matching_brace(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0;
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'{' { depth += 1; }
        if bytes[i] == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}
