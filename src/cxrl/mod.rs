// canvasx-runtime/src/cxrl/mod.rs
//
// CXRL — CanvasX Runtime Library
//
// A reusable component library format. Libraries contain:
// - Reusable node subtrees (components)
// - Shared styles / themes
// - Shared animation presets
// - Common assets (icons, fonts, textures)
//
// Libraries can be referenced by .cxrd documents and .cxrp packages.
// File extension: .cxrl

pub mod manifest;
pub mod loader;
pub mod builder;
