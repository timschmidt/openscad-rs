#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let source = support::bounded_source(data);
    match openscad_rs::parse(&source) {
        Ok(file) => {
            support::validate_ast(&source, &file);
            assert_eq!(openscad_rs::parse(&source).ok().as_ref(), Some(&file));
        }
        Err(error) => {
            support::validate_error(&source, &error);
            let _ = error.to_string();
            let _ = format!("{error:?}");
        }
    }
});
