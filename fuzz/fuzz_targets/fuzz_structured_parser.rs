#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some(source) = support::structured_program(data) else {
        return;
    };
    let file = openscad_rs::parse(&source)
        .unwrap_or_else(|error| panic!("generated valid source did not parse: {error}\n{source}"));
    support::validate_ast(&source, &file);
});
