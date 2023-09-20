use std::{io, path::PathBuf};

use clap::{arg, Command, ArgMatches};
use rcgen::generate_simple_self_signed;

mod run;

fn cli() -> Command {
    Command::new("pyne")
        .about("A personal notes server")
        .subcommand_required(true)
        // .arg_required_else_help(true)
        // .allow_external_subcommands(true)
        .subcommand(
            Command::new("new")
                .about("Create a new pyne server")
                .arg(arg!(name: <NAME> "Name of the new server"))
                .arg_required_else_help(true),
        )
        .subcommand(
            Command::new("run")
                .about("Run a pyne server")
                // .arg(arg!(path: <PATH> "Path to the server directory").default_value(".").value_parser(clap::value_parser!(PathBuf)))
                .arg(arg!(port: <PORT> "Server listening port"))
                .arg(arg!(path: <PATH> "Server instance directory").required(false).value_parser(clap::value_parser!(PathBuf)).default_value("."))
        )
}

#[tokio::main]
async fn main() {
    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("new", matches)) => cmd_new(matches),
        Some(("run", matches)) => run::start(matches).await.unwrap(),
        _ => unreachable!(),
    };
}

fn cmd_new(args: &ArgMatches) {
    let name = args.get_one::<String>("name").expect("Missing server name in `new` command.");

    // Create server directory.
    std::fs::create_dir(name).unwrap();

    // Create notes directory.
    std::fs::create_dir(format!("{name}/notes")).unwrap();

    // Generate the TLS files.
    gen_cert(format!("{name}/server.crt").into(), format!("{name}/server.key").into()).unwrap();
    
    println!("Created server (instance) `{name}` at `./{name}/*`");
}

fn gen_cert(certfile: PathBuf, keyfile: PathBuf) -> io::Result<()> {
    let subject_alt_names: &[_] = &["tls-server".to_string()];
    let cert = generate_simple_self_signed(subject_alt_names).unwrap();

    if let Some(dir) = certfile.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(certfile, cert.serialize_pem().unwrap())?;

    if let Some(dir) = keyfile.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(keyfile, cert.serialize_private_key_pem())?;

    Ok(())
}
