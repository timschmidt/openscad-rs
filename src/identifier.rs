//! The pinned upstream Unicode16 NFC and UTS39 identifier profile.

use unicode_normalization::UnicodeNormalization;
use unicode_security::GeneralSecurityProfile;
use unicode_xid::UnicodeXID;

// A Unicode-data upgrade is a language change, not an incidental dependency
// update. Keep the normalizer and both character properties on upstream's pin.
const _: () = assert!(
    unicode_normalization::UNICODE_VERSION.0 == 16
        && unicode_normalization::UNICODE_VERSION.1 == 0
        && unicode_normalization::UNICODE_VERSION.2 == 0
        && unicode_security::UNICODE_VERSION.0 == 16
        && unicode_security::UNICODE_VERSION.1 == 0
        && unicode_security::UNICODE_VERSION.2 == 0
        && unicode_xid::UNICODE_VERSION.0 == 16
        && unicode_xid::UNICODE_VERSION.1 == 0
        && unicode_xid::UNICODE_VERSION.2 == 0
);

/// Validate the non-ASCII token route after NFC, as upstream does. The lexer
/// keeps its existing ASCII and legacy digit-leading rules independently.
pub fn unicode_identifier_is_valid(source: &str) -> bool {
    let mut characters = source.nfc();
    characters.next().is_some_and(identifier_start) && characters.all(identifier_continue)
}

fn identifier_start(character: char) -> bool {
    if character.is_ascii() {
        character.is_ascii_alphabetic() || matches!(character, '_' | '$')
    } else {
        character.is_xid_start() && character.identifier_allowed()
    }
}

fn identifier_continue(character: char) -> bool {
    if character.is_ascii() {
        character.is_ascii_alphanumeric() || character == '_'
    } else {
        character.is_xid_continue() && character.identifier_allowed()
    }
}

/// Normalize only an AST name. Source text, token byte spans, string values
/// and include/use paths keep their original representations.
pub fn normalized_identifier(source: &str) -> String {
    if source.is_ascii() {
        source.to_owned()
    } else {
        source.nfc().collect()
    }
}
