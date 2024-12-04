use std::path::PathBuf;

use clap::Parser;
use deployment::SecretDb;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the secret database file
    #[arg(short, long)]
    secret_db: PathBuf,
}

fn main() {
    let args = Args::parse();

    // new will generate the global secrets if it doesn't exist
    let secret_db = SecretDb::new(args.secret_db)
        .expect("could not initialize the secret database");

    secret_db.generate_global_secret()
        .expect("failed to generate global secrets");
}
