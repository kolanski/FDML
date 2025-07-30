use fdml::{Cli, CommandRunner, error::print_error};
use std::process;

fn main() {
    let cli = Cli::parse_args();
    let runner = CommandRunner::new(cli.verbose);
    
    if let Err(error) = runner.run(cli) {
        print_error(&error);
        process::exit(1);
    }
}