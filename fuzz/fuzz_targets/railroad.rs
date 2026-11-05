//! Renders arbitrary text as a railroad diagram, in each of Mermaid's four notations. Any
//! panic is a bug: untrusted input must only ever produce `Err`. Output must be deterministic
//! and within the output limit.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|body: &str| {
    for keyword in [
        "railroad-beta",
        "railroad-ebnf-beta",
        "railroad-abnf-beta",
        "railroad-peg-beta",
    ] {
        let src = format!("{keyword}\n{body}");
        let options = pele_rs::RenderOptions::default();
        let first = pele_rs::render(&src, &options);
        if let Ok(rendered) = &first {
            assert!(rendered.svg.chars().count() <= options.limits.output);
            assert!(!rendered.svg.contains("<script"));
        }
        assert_eq!(first, pele_rs::render(&src, &options));
    }
});
