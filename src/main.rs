use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        arch_sense::print_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--apply") {
        return arch_sense::apply_saved_config();
    }

    arch_sense::run()
}
