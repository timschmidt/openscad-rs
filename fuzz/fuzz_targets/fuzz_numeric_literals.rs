#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;
use openscad_rs::lexer::lex_with_errors;
use openscad_rs::token::Token;

fn digits(data: &[u8], radix: u8) -> String {
    data.iter()
        .map(|byte| {
            let digit = byte % radix;
            if digit < 10 {
                char::from(b'0' + digit)
            } else {
                char::from(b'a' + digit - 10)
            }
        })
        .collect()
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let data = &data[..data.len().min(4 * 1024)];
    let split = 1 + usize::from(data[0]) % data.len();
    let left = digits(&data[..split], 10);
    let right = digits(&data[split..], 10);
    let nonempty_right = if right.is_empty() { "0" } else { &right };
    let literal = match data[0] % 6 {
        0 => left,
        1 => format!("{left}.{nonempty_right}"),
        2 => format!(".{left}"),
        3 => format!("{left}e+{nonempty_right}"),
        4 => format!("0x{}", digits(data, 16)),
        _ => format!("{left}e-{nonempty_right}"),
    };

    let (tokens, errors) = lex_with_errors(&literal);
    for span in &errors {
        support::assert_span(&literal, *span);
    }
    if errors.is_empty() {
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].0, Token::Number(_)));
        assert_eq!(tokens[0].1, openscad_rs::Span::new(0, literal.len()));
        assert!(openscad_rs::parse(&format!("value = {literal};")).is_ok());
    } else {
        assert!(openscad_rs::parse(&format!("value = {literal};")).is_err());
    }
});
