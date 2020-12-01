//! Integration tests

/// RPC integration tests.
///
/// These are all ignored by default, since they test against running
/// `tendermint node --proxy_app=kvstore`. They can be run using:
///
/// ```
/// cargo test -- --ignored
/// ```
mod rpc {
    use std::cmp::min;

    use tendermint_rpc::{Client, HttpClient, Id, Order, SubscriptionClient, WebSocketClient};

    use futures::StreamExt;
    use std::convert::TryFrom;
    use tendermint::abci::Log;
    use tendermint::abci::{Code, Transaction};
    use tendermint::block::Height;
    use tendermint::merkle::simple_hash_from_byte_vectors;
    use tendermint_rpc::endpoint::tx_search::ResultTx;
    use tendermint_rpc::event::{Event, EventData, TxInfo};
    use tendermint_rpc::query::{EventType, Query};
    use tokio::time::Duration;

    /// Get the address of the local node
    pub fn localhost_rpc_client() -> HttpClient {
        HttpClient::new("tcp://127.0.0.1:26657".parse().unwrap()).unwrap()
    }

    /// `/health` endpoint
    #[tokio::test]
    #[ignore]
    async fn health() {
        let result = localhost_rpc_client().health().await;

        assert!(result.is_ok(), "health check failed");
    }

    /// `/abci_info` endpoint
    #[tokio::test]
    #[ignore]
    async fn abci_info() {
        let abci_info = localhost_rpc_client().abci_info().await.unwrap();

        assert_eq!(abci_info.app_version, 1u64);
        assert_eq!(abci_info.data.is_empty(), false);
    }

    /// `/abci_query` endpoint
    #[tokio::test]
    #[ignore]
    async fn abci_query() {
        let key = "unpopulated_key".parse().unwrap();
        let abci_query = localhost_rpc_client()
            .abci_query(Some(key), vec![], None, false)
            .await
            .unwrap();

        assert_eq!(abci_query.code, Code::Ok);
        assert_eq!(abci_query.log, Log::from("does not exist"));
        assert_eq!(abci_query.info, String::new());
        assert_eq!(abci_query.index, 0);
        assert_eq!(&abci_query.key, &Vec::<u8>::new());
        assert!(&abci_query.key.is_empty());
        assert_eq!(abci_query.value, Vec::<u8>::new());
        assert!(abci_query.proof.is_none());
        assert!(abci_query.height.value() > 0);
        assert_eq!(abci_query.codespace, String::new());
    }

    /// `/block` endpoint
    #[tokio::test]
    #[ignore]
    async fn block() {
        let height = 1u64;
        let block_info = localhost_rpc_client()
            .block(Height::try_from(height).unwrap())
            .await
            .unwrap();

        assert!(block_info.block.last_commit.is_none());
        assert_eq!(block_info.block.header.height.value(), height);

        // Check for empty merkle root.
        // See: https://github.com/informalsystems/tendermint-rs/issues/562
        let computed_data_hash = simple_hash_from_byte_vectors(
            block_info
                .block
                .data
                .iter()
                .map(|t| t.to_owned().into())
                .collect(),
        );
        assert_eq!(
            computed_data_hash,
            block_info
                .block
                .header
                .data_hash
                .unwrap_or_default()
                .as_bytes()
        );
    }

    /// `/block_results` endpoint
    #[tokio::test]
    #[ignore]
    async fn block_results() {
        let height = 1u64;
        let block_results = localhost_rpc_client()
            .block_results(Height::try_from(height).unwrap())
            .await
            .unwrap();

        assert_eq!(block_results.height.value(), height);
        assert!(block_results.txs_results.is_none());
    }

    /// `/blockchain` endpoint
    #[tokio::test]
    #[ignore]
    async fn blockchain() {
        let max_height = 10u64;
        let blockchain_info = localhost_rpc_client()
            .blockchain(Height::from(1u32), Height::try_from(max_height).unwrap())
            .await
            .unwrap();

        assert_eq!(
            blockchain_info.block_metas.len() as u64,
            min(max_height, blockchain_info.last_height.value())
        );
    }

    /// `/commit` endpoint
    #[tokio::test]
    #[ignore]
    async fn commit() {
        let height = 1u64;
        let commit_info = localhost_rpc_client()
            .commit(Height::try_from(height).unwrap())
            .await
            .unwrap();

        assert_eq!(commit_info.signed_header.header.height.value(), height);
        assert_eq!(commit_info.canonical, true);
        assert_eq!(
            commit_info.signed_header.header.hash(),
            commit_info.signed_header.commit.block_id.hash
        );
    }

    /// `/genesis` endpoint
    #[tokio::test]
    #[ignore]
    async fn genesis() {
        let genesis = localhost_rpc_client().genesis().await.unwrap(); // https://github.com/tendermint/tendermint/issues/5549

        assert_eq!(
            genesis.consensus_params.validator.pub_key_types[0].to_string(),
            "ed25519"
        );
    }

    /// `/net_info` endpoint integration test
    #[tokio::test]
    #[ignore]
    async fn net_info() {
        let net_info = localhost_rpc_client().net_info().await.unwrap();

        assert!(net_info.listening);
    }

    /// `/status` endpoint integration test
    #[tokio::test]
    #[ignore]
    async fn status_integration() {
        let status = localhost_rpc_client().status().await.unwrap();

        // For lack of better things to test
        assert_eq!(status.validator_info.voting_power.value(), 10);
    }

    #[tokio::test]
    #[ignore]
    async fn subscription_interface() {
        let (client, driver) = WebSocketClient::new("tcp://127.0.0.1:26657".parse().unwrap())
            .await
            .unwrap();
        let driver_handle = tokio::spawn(async move { driver.run().await });
        let mut subs = client.subscribe(EventType::NewBlock.into()).await.unwrap();
        let mut ev_count = 5_i32;

        println!("Attempting to grab {} new blocks", ev_count);
        while let Some(res) = subs.next().await {
            let ev = res.unwrap();
            println!("Got event: {:?}", ev);
            ev_count -= 1;
            if ev_count < 0 {
                break;
            }
        }

        client.close().unwrap();
        let _ = driver_handle.await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn transaction_subscription() {
        // We run these sequentially wrapped within a single test to ensure
        // that Tokio doesn't execute them simultaneously. If they are executed
        // simultaneously, their submitted transactions interfere with each
        // other and one of them will (incorrectly) fail.
        simple_transaction_subscription().await;
        concurrent_subscriptions().await;
        tx_search().await;
    }

    async fn simple_transaction_subscription() {
        let (client, driver) = WebSocketClient::new("tcp://127.0.0.1:26657".parse().unwrap())
            .await
            .unwrap();
        let driver_handle = tokio::spawn(async move { driver.run().await });
        let mut subs = client.subscribe(EventType::Tx.into()).await.unwrap();
        // We use Id::uuid_v4() here as a quick hack to generate a random value.
        let mut expected_tx_values = (0..10_u32)
            .map(|_| Id::uuid_v4().to_string())
            .collect::<Vec<String>>();
        let broadcast_tx_values = expected_tx_values.clone();

        // We can clone the WebSocket client, because it's just a handle to the
        // driver.
        let inner_client = client.clone();
        tokio::spawn(async move {
            for (tx_count, val) in broadcast_tx_values.into_iter().enumerate() {
                let tx = format!("tx{}={}", tx_count, val);
                inner_client
                    .broadcast_tx_async(Transaction::from(tx.into_bytes()))
                    .await
                    .unwrap();
            }
        });

        println!(
            "Attempting to grab {} transaction events",
            expected_tx_values.len()
        );
        let mut cur_tx_id = 0_u32;

        while !expected_tx_values.is_empty() {
            let mut delay = tokio::time::delay_for(Duration::from_secs(3));
            tokio::select! {
                Some(res) = subs.next() => {
                    let ev = res.unwrap();
                    //println!("Got event: {:?}", ev);
                    let next_val = expected_tx_values.remove(0);
                    match ev.data {
                        EventData::Tx { tx_result } => match String::from_utf8(tx_result.tx) {
                            Ok(decoded_tx_str) => {
                                let decoded_tx_split = decoded_tx_str
                                    .split('=')
                                    .map(|s| s.to_string())
                                    .collect::<Vec<String>>();
                                assert_eq!(2, decoded_tx_split.len());

                                let key = decoded_tx_split.get(0).unwrap();
                                let val = decoded_tx_split.get(1).unwrap();
                                println!("Got tx: {}={}", key, val);
                                assert_eq!(format!("tx{}", cur_tx_id), *key);
                                assert_eq!(next_val, *val);
                            }
                            Err(e) => panic!("Failed to convert decoded tx to string: {}", e),
                        },
                        _ => panic!("Unexpected event type: {:?}", ev),
                    }
                    cur_tx_id += 1;
                },
                _ = &mut delay => panic!("Timed out waiting for an event"),
            }
        }

        client.close().unwrap();
        let _ = driver_handle.await.unwrap();
    }

    async fn concurrent_subscriptions() {
        let (client, driver) = WebSocketClient::new("tcp://127.0.0.1:26657".parse().unwrap())
            .await
            .unwrap();
        let driver_handle = tokio::spawn(async move { driver.run().await });
        let new_block_subs = client.subscribe(EventType::NewBlock.into()).await.unwrap();
        let tx_subs = client.subscribe(EventType::Tx.into()).await.unwrap();

        // We use Id::uuid_v4() here as a quick hack to generate a random value.
        let mut expected_tx_values = (0..10_u32)
            .map(|_| Id::uuid_v4().to_string())
            .collect::<Vec<String>>();
        let broadcast_tx_values = expected_tx_values.clone();
        let mut expected_new_blocks = 5_i32;

        let inner_client = client.clone();
        tokio::spawn(async move {
            for (tx_count, val) in broadcast_tx_values.into_iter().enumerate() {
                let tx = format!("tx{}={}", tx_count, val);
                inner_client
                    .broadcast_tx_async(Transaction::from(tx.into_bytes()))
                    .await
                    .unwrap();
                tokio::time::delay_for(Duration::from_millis(100)).await;
            }
        });

        let mut combined_subs = futures::stream::select_all(vec![new_block_subs, tx_subs]);

        println!(
            "Attempting to receive {} transactions and {} new blocks",
            expected_tx_values.len(),
            expected_new_blocks
        );

        while expected_new_blocks > 0 && !expected_tx_values.is_empty() {
            let mut timeout = tokio::time::delay_for(Duration::from_secs(3));
            tokio::select! {
                Some(res) = combined_subs.next() => {
                    let ev: Event = res.unwrap();
                    println!("Got event: {:?}", ev);
                    match ev.data {
                        EventData::NewBlock { .. } => {
                            println!("Got new block event");
                            expected_new_blocks -= 1;
                        },
                        EventData::Tx { .. } => {
                            println!("Got new transaction event");
                            let _ = expected_tx_values.pop();
                        },
                        _ => panic!("Unexpected event received: {:?}", ev),
                    }
                },
                _ = &mut timeout => panic!("Timed out waiting for an event"),
            }
        }

        client.close().unwrap();
        let _ = driver_handle.await.unwrap();
    }

    async fn tx_search() {
        let rpc_client = localhost_rpc_client();
        let (mut subs_client, driver) =
            WebSocketClient::new("tcp://127.0.0.1:26657".parse().unwrap())
                .await
                .unwrap();
        let driver_handle = tokio::spawn(async move { driver.run().await });

        let tx = "tx_search_key=tx_search_value".to_string();
        let tx_info = broadcast_tx(
            &rpc_client,
            &mut subs_client,
            Transaction::from(tx.into_bytes()),
        )
        .await
        .unwrap();
        println!("Got tx_info: {:?}", tx_info);

        let res = rpc_client
            .tx_search(
                Query::eq("app.key", "tx_search_key"),
                true,
                1,
                1,
                Order::Ascending,
            )
            .await
            .unwrap();
        assert!(res.total_count > 0);
        // We don't have more than 1 page of results
        assert_eq!(res.total_count as usize, res.txs.len());
        // Find our transaction
        let txs = res
            .txs
            .iter()
            .filter(|tx| tx.height.value() == (tx_info.height as u64))
            .collect::<Vec<&ResultTx>>();
        assert_eq!(1, txs.len());
        assert_eq!(tx_info.tx, txs[0].tx.as_bytes());

        subs_client.close().unwrap();
        driver_handle.await.unwrap().unwrap();
    }

    async fn broadcast_tx(
        http_client: &HttpClient,
        websocket_client: &mut WebSocketClient,
        tx: Transaction,
    ) -> Result<TxInfo, tendermint_rpc::Error> {
        let mut subs = websocket_client.subscribe(EventType::Tx.into()).await?;
        let _ = http_client.broadcast_tx_async(tx.clone()).await?;
        let mut timeout = tokio::time::delay_for(Duration::from_secs(3));
        tokio::select! {
            Some(res) = subs.next() => {
                let ev = res?;
                match ev.data {
                    EventData::Tx { tx_result } => {
                        let tx_result_bytes: &[u8] = tx_result.tx.as_ref();
                        // Make sure we have the right transaction here
                        assert_eq!(tx.as_bytes(), tx_result_bytes);
                        Ok(tx_result)
                    },
                    _ => panic!("Unexpected event: {:?}", ev),
                }
            }
            _ = &mut timeout => panic!("Timed out waiting for transaction"),
        }
    }
}
