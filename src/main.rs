use anyhow::Result;
use arch_sense::cli::Cli;
use arch_sense::commands;
use arch_sense::keyboard::RusbTransport;
use arch_sense::machine::MachineRoot;
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let machine = MachineRoot::system();

    if cli.apply {
        return commands::apply_saved_config(&machine, &mut RusbTransport);
    }

    arch_sense::run(machine, RusbTransport)
}
