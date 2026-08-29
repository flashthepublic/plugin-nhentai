// The plugin is a cdylib, so its parser unit tests need a native integration
// harness rather than being linked with the Extism PDK host imports.
#![allow(dead_code)]

#[path = "../src/nhentai.rs"]
mod nhentai;
