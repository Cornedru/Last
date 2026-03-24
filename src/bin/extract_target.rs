use regex::Regex;

pub struct VmExtractor;

impl VmExtractor {
    pub fn extract_vm_switch_block(js_code: &str) -> Option<String> {
        let patterns = [
            r"switch\([A-Za-z0-9_()]+\[[A-Za-z0-9_()]+\]\)",
            r"switch\([A-Za-z0-9_()]+\)",
        ];

        for pattern in &patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(m) = re.find(js_code) {
                    let start = m.start();
                    if let Some(end) = Self::find_block_end(js_code, start) {
                        let block = &js_code[start..end];
                        if block.len() > 100 && block.len() < 100000 {
                            if block.contains("case") && block.contains("this[") {
                                return Some(block.to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn wrap_for_parser(vm_block: &str) -> String {
        format!(
            "function VM() {{\n{}\n}}",
            vm_block
        )
    }

    fn find_block_end(text: &str, start: usize) -> Option<usize> {
        let mut depth = 0;
        let mut in_string = false;
        let mut string_char = ' ';
        let mut prev_char = '\\';

        for (i, c) in text[start..].chars().enumerate() {
            if !in_string {
                match c {
                    '{' | '(' => {
                        depth += 1;
                    }
                    '}' | ')' => {
                        depth -= 1;
                        if depth <= 0 {
                            return Some(start + i + 1);
                        }
                    }
                    '"' | '\'' | '`' => {
                        in_string = true;
                        string_char = c;
                    }
                    _ => {}
                }
            } else {
                if c == string_char && prev_char != '\\' {
                    in_string = false;
                }
            }
            prev_char = c;
        }
        None
    }

    pub fn find_vm_while_wrapper(js_code: &str) -> Option<String> {
        let while_patterns = [
            r"while\s*\([^)]+\)\s*\{[^}]*switch\s*\([^)]+\[[^)]+\]\)",
            r"while\s*\([^)]+\)\s*\{[^}]*switch\s*\([^)]+\)",
        ];

        for pattern in &while_patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(m) = re.find(js_code) {
                    let start = m.start();
                    if let Some(end) = Self::find_block_end(js_code, start) {
                        let block = &js_code[start..end];
                        if block.len() > 100 && block.len() < 100000 {
                            return Some(block.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    pub fn extract_all_vm_candidates(js_code: &str) -> Vec<VmCandidate> {
        let mut candidates = Vec::new();

        let switch_re = Regex::new(r"switch\([A-Za-z0-9_()]+\[[A-Za-z0-9_()]+\]\)|switch\([A-Za-z0-9_()]+\)").ok();

        if let Some(re) = switch_re {
            for m in re.find_iter(js_code) {
                let start = m.start();
                if let Some(end) = Self::find_block_end(js_code, start) {
                    let block = &js_code[start..end];
                    let case_count = Self::count_cases(block);

                    if block.len() > 100
                        && block.len() < 100000
                        && case_count > 5
                        && block.contains("this[")
                    {
                        candidates.push(VmCandidate {
                            start,
                            end,
                            block: block.to_string(),
                            case_count,
                        });
                    }
                }
            }
        }

        candidates.sort_by(|a, b| b.case_count.cmp(&a.case_count));
        candidates
    }

    fn count_cases(block: &str) -> usize {
        let case_re = Regex::new(r"(?i)case\s+['\"]?\d+['\"]?\s*:").ok();
        case_re.map(|re| re.find_iter(block).count()).unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct VmCandidate {
    pub start: usize,
    pub end: usize,
    pub block: String,
    pub case_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_vm_switch_block() {
        let js = r#"
        function test() {
            while (this.g < 256) {
                switch (this[this.g ^ 52]) {
                    case 0: return;
                    case 1: this.g += 2; break;
                }
            }
        }
        "#;

        let result = VmExtractor::extract_vm_switch_block(js);
        assert!(result.is_some());
        let block = result.unwrap();
        assert!(block.contains("switch"));
        assert!(block.contains("this[this.g ^ 52]"));
        assert!(block.contains("case 0"));
    }

    #[test]
    fn test_wrap_for_parser() {
        let block = "switch (x) { case 0: break; }";
        let wrapped = VmExtractor::wrap_for_parser(block);
        assert!(wrapped.contains("function VM()"));
        assert!(wrapped.contains(block));
    }

    #[test]
    fn test_count_cases() {
        let block = "case 0: case 1: case 2: break;";
        assert_eq!(VmExtractor::count_cases(block), 3);
    }
}
