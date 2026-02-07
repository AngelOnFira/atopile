#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings
    if let Ok(input) = std::str::from_utf8(data) {
        // Test the full lex -> parse pipeline
        let (tokens, _errors) = ato_lexer::lex(input);

        // Even if lexing produced errors, try parsing the original source
        // (the parser does its own lexing internally)
        let _ = ato_parser::parse_with_recovery(input);

        // Also verify that tokens don't cause issues when inspected
        for token in &tokens {
            let _ = format!("{:?}", token);
        }
    }
});
