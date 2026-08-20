#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;
use openscad_rs::lexer::{extract_include_path, lex, lex_with_errors};

fuzz_target!(|data: &[u8]| {
    let source = support::bounded_source(data);
    let tokens = lex(&source);
    let (reported_tokens, errors) = lex_with_errors(&source);
    assert_eq!(tokens, reported_tokens);

    let mut previous_end = 0;
    for (_, span) in &tokens {
        support::assert_span(&source, *span);
        assert!(span.start >= previous_end);
        previous_end = span.end;
        let slice = &source[span.start..span.end];
        let path = extract_include_path(slice);
        assert!(path.len() <= slice.len());
    }
    for span in errors {
        support::assert_span(&source, span);
    }
});
