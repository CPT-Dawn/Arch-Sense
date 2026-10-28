use anyhow::Result;
use arch_sense::cli::{Cli, Command};
use arch_sense::commands;
use arch_sense::doctor::Membership;
use arch_sense::keyboard::RusbTransport;
use arch_sense::machine::MachineRoot;
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let machine = MachineRoot::system();

    if cli.command == Some(Command::Doctor) {
        let membership = Membership::current(&machine);
        let code = commands::doctor(&machine, membership, &mut std::io::stdout().lock())?;
        std::process::exit(code);
    }

    if cli.apply {
        return commands::apply_saved_config(&machine, &mut RusbTransport);
    }

    arch_sense::run(machine, RusbTransport)
}
