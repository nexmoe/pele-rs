//! Renders arbitrary text as a flowchart. Any panic is a bug: untrusted input must only ever
//! produce `Err`. Output must be deterministic and within the output limit.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|body: &str| {
    let src = format!("flowchart TD\n{body}");
    let options = pele_rs::RenderOptions::default();
    let first = pele_rs::render(&src, &options);
    if let Ok(rendered) = &first {
        assert!(rendered.svg.chars().count() <= options.limits.output);
        assert!(!rendered.svg.contains("<script"));
    }
    assert_eq!(first, pele_rs::render(&src, &options));
});
