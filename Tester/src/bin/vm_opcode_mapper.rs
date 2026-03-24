use anyhow::Result;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║  VM OPCODE MAPPER - Deobfuscation Pipeline             ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        println!("Usage:");
        println!("  vm_opcode_mapper <js_file> [--no-deobfuscate] [output_json]");
        println!("  vm_opcode_mapper --fetch <url> [--no-deobfuscate] [output_json]");
        println!();
        println!("Options:");
        println!("  --no-deobfuscate  Skip deobfuscation pass");
        println!();
        println!("Examples:");
        println!("  vm_opcode_mapper function.js");
        println!("  vm_opcode_mapper function.js --no-deobfuscate");
        std::process::exit(1);
    }

    let js_code = if args[1] == "--fetch" {
        if args.len() < 3 {
            anyhow::bail!("--fetch requires a URL");
        }
        let url = &args[2];
        println!("[FETCH] Downloading: {}", url);
        let response = ureq::get(url)
            .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .call()?;
        response.into_string()?
    } else {
        let path = &args[1];
        println!("[READ] Loading: {}", path);
        std::fs::read_to_string(path)?
    };

    let skip_deobfuscate = args.contains(&"--no-deobfuscate".to_string());
    
    let clean_js = if skip_deobfuscate {
        println!("[DEOBF] Skipping deobfuscation (--no-deobfuscate flag)\n");
        js_code.clone()
    } else {
        println!("[DEOBF] Running string decoder (Ma())...");
        let output = cf::deobfuscator::MaStringDecoder::decode(&js_code);
        
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
    
    if output_path != "function_clean.js" {
        cf::solver::vm_opcode_mapper::export_opcode_map(&mapping, output_path)?;
        println!("\n[SAVE] Mapping exported to: {}", output_path);
    }

    Ok(())
}
