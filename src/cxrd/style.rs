// canvasx-runtime/src/cxrd/style.rs
//
// Computed style properties for CXRD nodes.
// These are fully resolved — no cascading, no inheritance at render time.
// The compiler resolves all CSS into computed styles during compilation.

use serde::{Serialize, Deserialize};
use crate::cxrd::value::{Color, Dimension, EdgeInsets, CornerRadii};

/// Display mode for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Display {
    Flex,
    Grid,
    Block,
    InlineBlock,
    None,
}

impl Default for Display {
    fn default() -> Self {
        Display::Block
    }
}

/// Flex direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlexDirection {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

impl Default for FlexDirection {
    fn default() -> Self {
        FlexDirection::Row
    }
}

/// Flex wrap mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

impl Default for FlexWrap {
    fn default() -> Self {
        FlexWrap::NoWrap
    }
}

/// Justify-content values (main-axis alignment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JustifyContent {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

impl Default for JustifyContent {
    fn default() -> Self {
        JustifyContent::FlexStart
    }
}

/// Align-items values (cross-axis alignment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignItems {
    FlexStart,
    FlexEnd,
    Center,
    Stretch,
    Baseline,
}

impl Default for AlignItems {
    fn default() -> Self {
        AlignItems::Stretch
    }
}

/// Align-self (per-child cross-axis override).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlignSelf {
    Auto,
    FlexStart,
    FlexEnd,
    Center,
    Stretch,
}

impl Default for AlignSelf {
    fn default() -> Self {
        AlignSelf::Auto
    }
}

/// Positioning mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Position {
    Relative,
    Absolute,
    Fixed,
}

impl Default for Position {
    fn default() -> Self {
        Position::Relative
    }
}

/// Overflow behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
}

impl Default for Overflow {
    fn default() -> Self {
        Overflow::Visible
    }
}

/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Text transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextTransform {
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

impl Default for TextTransform {
    fn default() -> Self {
        TextTransform::None
    }
}

/// A CSS grid track size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GridTrackSize {
    Auto,
    Px(f32),
    Percent(f32),
    Fr(f32),
    MinContent,
    MaxContent,
}

impl Default for TextAlign {
    fn default() -> Self {
        TextAlign::Left
    }
}

/// Font weight (100–900).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontWeight(pub u16);

impl Default for FontWeight {
    fn default() -> Self {
        FontWeight(400)
    }
}

/// Background specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Background {
    None,
    Solid(Color),
    LinearGradient {
        angle_deg: f32,
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        stops: Vec<GradientStop>,
    },
    Image {
        /// Index into the CXRD asset table.
        asset_index: u32,
    },
}

impl Default for Background {
    fn default() -> Self {
        Background::None
    }
}

/// A gradient color stop.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub color: Color,
    pub position: f32, // 0.0–1.0
}

/// Box shadow.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub color: Color,
    pub inset: bool,
}

/// CSS transition definition (compiled from CSS `transition` shorthand).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionDef {
    pub property: String,
    pub duration_ms: f32,
    pub delay_ms: f32,
    pub easing: EasingFunction,
}

/// Easing function for transitions and animations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EasingFunction {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
}

impl Default for EasingFunction {
    fn default() -> Self {
        EasingFunction::Ease
    }
}

/// The fully-computed style for a CXRD node.
/// Every field is resolved — no inheritance lookups, no cascade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComputedStyle {
    // --- Layout ---
    pub display: Display,
    pub position: Position,
    pub overflow: Overflow,

    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub min_height: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,

    pub margin: EdgeInsetsD,
    pub padding: EdgeInsetsD,

    // --- Flex ---
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_self: AlignSelf,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
    pub gap: f32,

    // --- Position offsets (for absolute / fixed) ---
    pub top: Dimension,
    pub right: Dimension,
    pub bottom: Dimension,
    pub left: Dimension,

    // --- Visual ---
    pub background: Background,
    pub border_color: Color,
    pub border_width: EdgeInsets,
    pub border_radius: CornerRadii,
    pub box_shadow: Vec<BoxShadow>,
    pub backdrop_blur: f32,
    pub transform_scale: f32,
    pub opacity: f32,

    // --- Grid ---
    pub grid_template_columns: Vec<GridTrackSize>,
    pub grid_template_rows: Vec<GridTrackSize>,
    pub grid_column_start: i32,  // 0 = auto, positive = line number, negative = from end
    pub grid_column_end: i32,    // 0 = auto, -1 = last line
    pub grid_row_start: i32,
    pub grid_row_end: i32,

    // --- Typography ---
    pub color: Color,
    pub font_family: String,
    pub font_size: f32,      // px, resolved
    pub font_weight: FontWeight,
    pub line_height: f32,    // multiplier
    pub text_align: TextAlign,
    pub letter_spacing: f32,
    pub text_transform: TextTransform,

    // --- Transitions ---
    pub transitions: Vec<TransitionDef>,

    // --- Z-index (for stacking context) ---
    pub z_index: i32,
}

/// Edge insets in dimension form (before resolution to px).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgeInsetsD {
    pub top: Dimension,
    pub right: Dimension,
    pub bottom: Dimension,
    pub left: Dimension,
}

impl Default for EdgeInsetsD {
    fn default() -> Self {
        Self {
            top: Dimension::Px(0.0),
            right: Dimension::Px(0.0),
            bottom: Dimension::Px(0.0),
            left: Dimension::Px(0.0),
        }
    }
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::default(),
            position: Position::default(),
            overflow: Overflow::default(),
            width: Dimension::Auto,
            height: Dimension::Auto,
            min_width: Dimension::Px(0.0),
            min_height: Dimension::Px(0.0),
            max_width: Dimension::Auto,
            max_height: Dimension::Auto,
            margin: EdgeInsetsD::default(),
            padding: EdgeInsetsD::default(),
            flex_direction: FlexDirection::default(),
            flex_wrap: FlexWrap::default(),
            justify_content: JustifyContent::default(),
            align_items: AlignItems::default(),
            align_self: AlignSelf::default(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dimension::Auto,
            gap: 0.0,
            grid_template_columns: Vec::new(),
            grid_template_rows: Vec::new(),
            grid_column_start: 0,
            grid_column_end: 0,
            grid_row_start: 0,
            grid_row_end: 0,
            top: Dimension::Auto,
            right: Dimension::Auto,
            bottom: Dimension::Auto,
            left: Dimension::Auto,
            background: Background::default(),
            border_color: Color::TRANSPARENT,
            border_width: EdgeInsets::default(),
            border_radius: CornerRadii::default(),
            box_shadow: Vec::new(),
            backdrop_blur: 0.0,
            transform_scale: 1.0,
            opacity: 1.0,
            color: Color::WHITE,
            font_family: String::new(),
            font_size: 16.0,
            font_weight: FontWeight::default(),
            line_height: 1.5,
            text_align: TextAlign::default(),
            letter_spacing: 0.0,
            text_transform: TextTransform::default(),
            transitions: Vec::new(),
            z_index: 0,
        }
    }
}
