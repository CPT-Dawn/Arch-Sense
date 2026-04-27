use anyhow::Result;
use clap::Parser;
use arch_sense::cli::Cli;
use arch_sense::commands;

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.doctor {
        return commands::print_permission_report();
    }

    if cli.install_permissions_root {
        return commands::install_permissions_as_root();
    }

    if cli.install_permissions {
        return commands::install_permissions();
    }

    if cli.apply_permissions {
        return commands::apply_permissions();
    }

    if cli.apply {
        return commands::apply_saved_config();
    }

    arch_sense::run()
}
