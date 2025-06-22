use std::path::PathBuf;

use another_rusty_world::engine::Engine;
use clap::Parser;
use log::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "./another_world")]
    data_dir: String,
}

fn main() {
    let args = Args::parse();
    env_logger::init();

    if let Err(e) = Engine::run(PathBuf::from(args.data_dir)) {
        error!("Engine terminated abruptly. Error: {:?}", e);
        return;
    }
    info!("Execution terminated successfully");
}
