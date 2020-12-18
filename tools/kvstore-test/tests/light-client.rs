//! Light Client integration tests.
//!
/// If you have a kvstore app running on 127.0.0.1:26657,
/// these can be run using:
///
///     cargo test
///
/// Or else, if you have docker installed, you can tell the tests to run an endpoint,
/// by running:
///
///     cargo make
///
/// (Make sure you install cargo-make using `cargo install cargo-make` first.)
///

use tendermint_light_client::{
    builder::LightClientBuilder,
    builder::SupervisorBuilder,
    components::io::AtHeight,
    components::io::Io,
    components::io::IoError,
    components::io::ProdIo,
    evidence::{Evidence, EvidenceReporter},
    light_client,
    store::memory::MemoryStore,
    store::LightStore,
    supervisor::{Handle, Instance},
    types::{PeerId, Status, TrustThreshold},
};

use tendermint::abci::transaction::Hash as TxHash;
use tendermint::net;
use tendermint_rpc as rpc;

use std::time::Duration;

fn make_instance(
    peer_id: PeerId,
    options: light_client::Options,
    address: net::Address,
) -> Instance {
    let rpc_client = rpc::HttpClient::new(address).unwrap();
    let io = ProdIo::new(peer_id, rpc_client.clone(), Some(Duration::from_secs(2)));
    let latest_block = io.fetch_light_block(AtHeight::Highest).unwrap();

    let mut light_store = Box::new(MemoryStore::new());
    light_store.insert(latest_block, Status::Trusted);

    LightClientBuilder::prod(
        peer_id,
        rpc_client,
        light_store,
        options,
        Some(Duration::from_secs(2)),
    )
    .trust_from_store()
    .unwrap()
    .build()
}

struct TestEvidenceReporter;

#[contracts::contract_trait]
impl EvidenceReporter for TestEvidenceReporter {
    fn report(&self, evidence: Evidence, peer: PeerId) -> Result<TxHash, IoError> {
        panic!(
            "unexpected fork detected for peer {} with evidence: {:?}",
            peer, evidence
        );
    }
}

#[test]
fn sync() {
    let primary: PeerId = "BADFADAD0BEFEEDC0C0ADEADBEEFC0FFEEFACADE".parse().unwrap();
    let witness: PeerId = "CEFEEDBADFADAD0C0CEEFACADE0ADEADBEEFC0FF".parse().unwrap();

    // Because our CI infrastructure can only spawn a single Tendermint node at the moment,
    // we run this test against this very node as both the primary and witness.
    // In a production environment, one should make sure that the primary and witness are
    // different nodes, and check that the configured peer IDs match the ones returned
    // by the nodes.
    let node_address: tendermint::net::Address = "tcp://127.0.0.1:26657".parse().unwrap();

    let options = light_client::Options {
        trust_threshold: TrustThreshold {
            numerator: 1,
            denominator: 3,
        },
        trusting_period: Duration::from_secs(60 * 60), // 60 minutes
        clock_drift: Duration::from_secs(5 * 60),      // 5 minutes
    };

    let primary_instance = make_instance(primary, options, node_address.clone());
    let witness_instance = make_instance(witness, options, node_address.clone());

    let supervisor = SupervisorBuilder::new()
        .primary(primary, node_address.clone(), primary_instance)
        .witness(witness, node_address, witness_instance)
        .build_prod();

    let handle = supervisor.handle();
    std::thread::spawn(|| supervisor.run());

    let max_iterations: usize = 10;

    for i in 1..=max_iterations {
        println!("[info ] - iteration {}/{}", i, max_iterations);

        match handle.verify_to_highest() {
            Ok(light_block) => {
                println!("[info ] synced to block {}", light_block.height());
            }
            Err(err) => {
                println!("[error] sync failed: {}", err);
                panic!("failed to sync to highest: {}", err);
            }
        }

        std::thread::sleep(Duration::from_millis(800));
    }
}
