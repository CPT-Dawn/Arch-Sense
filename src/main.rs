use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        arch_sense::print_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--doctor") {
        return arch_sense::print_permission_report();
    }

    if args.iter().any(|a| a == "--install-permissions-root") {
        return arch_sense::install_permissions_as_root();
    }

    if args.iter().any(|a| a == "--install-permissions") {
        return arch_sense::install_permissions();
    }

    if args.iter().any(|a| a == "--apply-permissions") {
        return arch_sense::apply_permissions();
    }

    if args.iter().any(|a| a == "--apply") {
        return arch_sense::apply_saved_config();
    }

    arch_sense::run()
}
