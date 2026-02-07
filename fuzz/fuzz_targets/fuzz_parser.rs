#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only fuzz valid UTF-8 strings — the parser expects &str
    if let Ok(input) = std::str::from_utf8(data) {
        // parse_with_recovery should never panic, only return errors
        let _ = ato_parser::parse_with_recovery(input);
    }
});
