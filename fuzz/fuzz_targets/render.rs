//! Renders arbitrary text. Any panic is a bug: untrusted input must only ever produce `Err`.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|src: &str| {
    let options = pele_rs::RenderOptions::default();
    if let Ok(rendered) = pele_rs::render(src, &options) {
        assert!(rendered.svg.chars().count() <= options.limits.output);
    }
    let _ = pele_rs::detect_type(src);
});
