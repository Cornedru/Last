pub struct MaStringDecoder;

impl MaStringDecoder {
    pub fn decode(code: &str) -> String {
        let strings = Self::extract_strings(code);

        if strings.is_empty() {
            return code.to_string();
        }

        let mut result = code.to_string();
        for (idx, s) in strings.iter().enumerate() {
            let pattern = format!("Ma({})", idx);
            let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
            result = result.replace(&pattern, &format!("\"{}\"", escaped));
        }

        result
    }

    fn extract_strings(code: &str) -> Vec<String> {
        eprintln!("[MaStringDecoder] Code length: {}", code.len());

        // Find the last occurrence of .split("~") or .split('~') with a long single-quoted string before it
        // The string starts with 'moVRS and ends with axCwh9'
        if let Some(mo_start) = code.find("'moVRS") {
            if let Some(split_pos) = code[mo_start..].find(".split(") {
                let actual_split_pos = mo_start + split_pos;
                eprintln!(
                    "[MaStringDecoder] Found 'moVRS at {}, .split at {}",
                    mo_start, actual_split_pos
                );

                let string_content = &code[mo_start + 1..actual_split_pos - 1]; // +1 to skip ', -1 to exclude '
                eprintln!(
                    "[MaStringDecoder] String content length: {}",
                    string_content.len()
                );

                if string_content.len() > 5000 {
                    // Extract separator
                    let after_split =
                        &code[actual_split_pos..(actual_split_pos + 20).min(code.len())];
                    let sep_start = after_split.find('(').map(|p| p + 1).unwrap_or(6);
                    let sep_rest = &after_split[sep_start..];
                    let sep_char = if sep_rest.starts_with('"') || sep_rest.starts_with('\'') {
                        let quote = sep_rest.chars().next().unwrap();
                        if let Some(end) = sep_rest[1..].find(quote) {
                            &sep_rest[1..end + 1]
                        } else {
                            "~"
                        }
                    } else {
                        "~"
                    };

                    eprintln!("[MaStringDecoder] Extracting with separator '{}'", sep_char);

                    let strings: Vec<String> = string_content
                        .split(sep_char)
                        .map(|s| s.to_string())
                        .collect();
                    eprintln!("[MaStringDecoder] Extracted {} strings", strings.len());
                    return strings;
                }
            }
        }

        eprintln!("[MaStringDecoder] Could not find string array pattern");
        Vec::new()
    }
}
