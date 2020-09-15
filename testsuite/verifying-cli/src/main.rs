use libra_types::{
    waypoint::Waypoint,
    chain_id::ChainId,
};
use cli::{
    client_proxy::ClientProxy,
};

use chrono::{
    prelude::{Utc},
    DateTime,
};

use structopt::StructOpt;
use std::{
    time::{Duration, UNIX_EPOCH},
};


#[derive(StructOpt)]
struct Args {
        /// Full URL address to connect to - should include port number, if applicable
        #[structopt(short = "u", long)]
        pub url: String,
    #[structopt(
        name = "waypoint",
        long,
        help = "Explicitly specify the waypoint to use"
    )]
    pub waypoint: Option<Waypoint>,
}

fn main() {
    let args = Args::from_args();
    let waypoint = args.waypoint.unwrap();
    println!("Working on URL : {}  \nwith waypoint: {}", args.url, waypoint.to_string());


    let mut client_proxy = ClientProxy::new(
        ChainId::new(2), // TESTNET = 2
        &args.url,
        "",
        "",
        "",
        false,
        None::<String>,
        None::<String>,
        waypoint,
    )
    .expect("Failed to construct client.");

    // Test connection to validator
    let block_metadata = client_proxy
        .test_validator_connection()
        .unwrap_or_else(|e| {
            panic!(
                "Not able to connect to validator at {}. Error: {}",
                args.url, e,
            )
        });

    let ledger_info_str = format!(
        "latest version = {}, timestamp = {}",
        block_metadata.version,
        DateTime::<Utc>::from(UNIX_EPOCH + Duration::from_micros(block_metadata.timestamp))
    );

    let state_proofs_result = client_proxy.test_trusted_connection();

    if !state_proofs_result.is_ok() {
        println!("Proofs did not validate");
    } else {
        println!("Proofs validated");
    }

    println!("ChainID: {}", client_proxy.chain_id);
    println!("Trusted State: {:#?}", client_proxy.client.trusted_state());
    println!("LedgerInfo: {:#?}", client_proxy.client.latest_epoch_change_li());

    let cli_info = format!(
        "Connected to validator at: {}, {}",
        args.url, ledger_info_str
    );

    println!("Cli info: {}", cli_info)

}