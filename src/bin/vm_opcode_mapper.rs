use anyhow;
use regex::Regex;
use std::env;

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║  VM OPCODE MAPPER - Deobfuscation Pipeline             ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        println!("Usage:");
        println!("  vm-opcode-mapper <js_file> [--no-deobfuscate] [--extract] [output_json]");
        println!("  vm-opcode-mapper --fetch <url> [--no-deobfuscate] [--extract] [output_json]");
        println!();
        println!("Options:");
        println!("  --no-deobfuscate  Skip deobfuscation pass");
        println!("  --extract         Extract VM switch block from obfuscated JS");
        println!();
        println!("Examples:");
        println!("  vm-opcode-mapper function.js");
        println!("  vm-opcode-mapper function.js --extract");
        println!("  vm-opcode-mapper function.js --no-deobfuscate");
        std::process::exit(1);
    }

    if let Err(e) = run(args) {
        eprintln!("[ERROR] {}", e);
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> anyhow::Result<()> {
    let mut pos = 1;
    let mut is_fetch = false;
    let mut skip_deobfuscate = false;
    let mut do_extract = false;
    let mut url_or_path = None;

    while pos < args.len() {
        match args[pos].as_str() {
            "--fetch" => {
                is_fetch = true;
                pos += 1;
                if pos < args.len() && !args[pos].starts_with("--") {
                    url_or_path = Some(args[pos].clone());
                    pos += 1;
                }
            }
            "--no-deobfuscate" => {
                skip_deobfuscate = true;
                pos += 1;
            }
            "--extract" => {
                do_extract = true;
                pos += 1;
            }
            _ => {
                url_or_path = Some(args[pos].clone());
                pos += 1;
            }
        }
    }

    if url_or_path.is_none() {
        anyhow::bail!("No input file or URL specified");
    }

    let js_code = if is_fetch {
        let url = url_or_path.unwrap();
        println!("[FETCH] Downloading: {}", url);
        let response = ureq::get(&url)
            .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .call()?;
        response.into_string()
            .map_err(|e| anyhow::anyhow!("Failed to read response: {}", e))?
    } else {
        let path = url_or_path.unwrap();
        println!("[READ] Loading: {}", path);
        std::fs::read_to_string(path)?
    };
    
    let clean_js = if do_extract {
        println!("[EXTRACT] Extracting VM switch block...");
        let candidates = extract_vm_candidates(&js_code);
        
        if candidates.is_empty() {
            anyhow::bail!("No VM switch candidates found in the JS code");
        }
        
        let best = &candidates[0];
        println!("[EXTRACT] Found {} candidate(s), using best ({} cases)", candidates.len(), best.case_count);
        println!("[EXTRACT] Block length: {} chars", best.block.len());
        
        let wrapped = wrap_for_parser(&best.block);
        println!("[EXTRACT] Wrapped for parser: {} chars\n", wrapped.len());
        
        std::fs::write("vm_extracted.js", &wrapped)?;
        println!("[SAVE] Extracted VM written to: vm_extracted.js\n");
        
        wrapped
    } else if skip_deobfuscate {
        println!("[DEOBF] Skipping deobfuscation (--no-deobfuscate flag)\n");
        js_code.clone()
    } else {
        println!("[DEOBF] Running deobfuscation pipeline...");
        let output = cf::deobfuscator::MaStringDecoder::decode(&js_code, true);
        
        let clean_file = "function_clean.js";
        std::fs::write(clean_file, &output)?;
        println!("[SAVE] Cleaned JS written to: {}\n", clean_file);
        
        output
    };
    
    println!("[PARSE] Analyzing AST...");
    let mapping = cf::solver::vm_opcode_mapper::analyze_vm_opcodes(&clean_js)?;
    
    println!("\n[RESULT] Opcode mapping summary:");
    println!("  Total opcodes: {}", mapping.opcode_to_type.len());
    println!("  State properties: {:?}", mapping.state_property_names);
    println!("  Heuristics applied: {}", mapping.heuristics_applied.len());
    
    println!("\n[DETAIL] Opcode breakdown:");
    for (opcode, instr_type) in mapping.opcode_to_type.iter() {
        println!("  {:3} -> {:?}", opcode, instr_type);
    }
    
    let output_path = args.last()
        .filter(|p| !p.starts_with("--"))
        .map(|s| s.as_str())
        .unwrap_or("opcode_mapping.json");
    
    if output_path != "function_clean.js" && output_path != "vm_extracted.js" {
        cf::solver::vm_opcode_mapper::export_opcode_map(&mapping, output_path)?;
        println!("\n[SAVE] Mapping exported to: {}", output_path);
    }

    Ok(())
}

fn wrap_for_parser(vm_block: &str) -> String {
    format!(
        "function VM() {{\n{}\n}}",
        vm_block
    )
}

fn extract_vm_candidates(js_code: &str) -> Vec<VmCandidate> {
    let mut candidates = Vec::new();

    let switch_re = Regex::new(r"switch\s*\([^)]+\[[^)]+\][^)]*\)");

    eprintln!("[DEBUG] Starting extraction, JS length: {}", js_code.len());
    eprintln!("[DEBUG] Regex compiled: {}", switch_re.is_ok());
    
    match switch_re {
        Ok(re) => {
        let test_match = re.find("switch(I[D++])");
        eprintln!("[DEBUG] Test match on 'switch(I[D++])': {:?}", test_match.map(|m| m.as_str()));
        
        let matches: Vec<_> = re.find_iter(js_code).collect();
        eprintln!("[DEBUG] Regex found {} switch matches", matches.len());
        
        for m in matches.iter().take(5) {
            eprintln!("[DEBUG] Match: {} chars at {}", m.as_str().len(), m.start());
        }
        
        for m in re.find_iter(js_code) {
            let start = m.start();
            eprintln!("[DEBUG] Checking match at position {}", start);
            if let Some(end) = find_block_end(js_code, start) {
                let block = &js_code[start..end];
                let case_count = count_cases(block);

                eprintln!("[DEBUG] Block: {} chars, {} cases", block.len(), case_count);

                if block.len() > 100
                    && block.len() < 100000
                    && case_count > 2
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
        },
        Err(e) => {
            eprintln!("[DEBUG] Regex error: {}", e);
        }
    }

    candidates.sort_by(|a, b| b.case_count.cmp(&a.case_count));
    candidates
}

fn find_block_end(text: &str, switch_start: usize) -> Option<usize> {
    let switch_patterns = [
        r"switch\s*\([^)]+\[[^)]+\][^)]*\)",
        r"switch\s*\([^)]+\)"
    ];
    
    let expr_end = switch_patterns.iter()
        .filter_map(|p| Regex::new(p).ok())
        .filter_map(|re| re.find(&text[switch_start..]))
        .map(|m| switch_start + m.end())
        .min();

    let body_start = match expr_end {
        Some(pos) => {
            text[switch_start..].find('{').map(|off| switch_start + off)
        }
        None => None
    }?;

    let mut depth = 0;
    let mut in_string = false;
    let mut string_char = ' ';
    let mut prev_char = '\\';

    for (i, c) in text[body_start..].chars().enumerate() {
        if !in_string {
            match c {
                '{' => {
                    depth += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth <= 0 {
                        return Some(body_start + i + 1);
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

fn count_cases(block: &str) -> usize {
    let case_re = Regex::new(r#"(?i)case\s*['"]?\d+['"]?\s*:"#).ok();
    case_re.map(|re| re.find_iter(block).count()).unwrap_or(0)
}

#[derive(Debug, Clone)]
struct VmCandidate {
    start: usize,
    end: usize,
    block: String,
    case_count: usize,
}
