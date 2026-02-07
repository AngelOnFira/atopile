#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings — the lexer expects &str
    if let Ok(input) = std::str::from_utf8(data) {
        // lex() should never panic, only return errors
        let _ = ato_lexer::lex(input);
    }
});
