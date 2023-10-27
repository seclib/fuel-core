use std::{str::FromStr, path::Path};
use bech32::{
    ToBase32,
    Variant::Bech32m,
};

use crate::{FUEL_BECH32_HRP, TESTNET_INITIAL_BALANCE};

use fuel_core_storage::Result as StorageResult;
use fuel_core_types::{fuel_types::{BlockHeight, Address, Bytes32}, fuel_vm::SecretKey, fuel_tx::UtxoId};

use itertools::Itertools;
use serde::{
    Deserialize,
    Serialize,
};
use serde_with::{
    serde_as,
    skip_serializing_none,
};

use super::{
    coin::CoinConfig,
    contract::ContractConfig,
    message::MessageConfig,
};

// TODO: do streaming deserialization to handle large state configs
#[serde_as]
#[skip_serializing_none]
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct StateConfig {
    /// Spendable coins
    pub coins: Option<Vec<CoinConfig>>,
    /// Contract state
    pub contracts: Option<Vec<ContractConfig>>,
    /// Messages from Layer 1
    pub messages: Option<Vec<MessageConfig>>,
}

impl StateConfig {
    pub fn generate_state_config<T>(db: T) -> StorageResult<Self>
    where
        T: ChainConfigDb,
    {
        Ok(StateConfig {
            coins: db.get_coin_config()?,
            contracts: db.get_contract_config()?,
            messages: db.get_message_config()?,
        })
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let contents = std::fs::read(path.as_ref().join("chain_state.json"))?;
        serde_json::from_slice(&contents).map_err(|e| {
            anyhow::Error::new(e).context(format!(
                "an error occurred while loading the chain parameters file"
            ))
        })
    }

    pub fn local_testnet() -> Self {
        // endow some preset accounts with an initial balance
        tracing::info!("Initial Accounts");
        let secrets = [
            "0xde97d8624a438121b86a1956544bd72ed68cd69f2c99555b08b1e8c51ffd511c",
            "0x37fa81c84ccd547c30c176b118d5cb892bdb113e8e80141f266519422ef9eefd",
            "0x862512a2363db2b3a375c0d4bbbd27172180d89f23f2e259bac850ab02619301",
            "0x976e5c3fa620092c718d852ca703b6da9e3075b9f2ecb8ed42d9f746bf26aafb",
            "0x7f8a325504e7315eda997db7861c9447f5c3eff26333b20180475d94443a10c6",
        ];
        let initial_coins = secrets
            .into_iter()
            .map(|secret| {
                let secret = SecretKey::from_str(secret).expect("Expected valid secret");
                let address = Address::from(*secret.public_key().hash());
                let bech32_data = Bytes32::new(*address).to_base32();
                let bech32_encoding =
                    bech32::encode(FUEL_BECH32_HRP, bech32_data, Bech32m).unwrap();
                tracing::info!(
                    "PrivateKey({:#x}), Address({:#x} [bech32: {}]), Balance({})",
                    secret,
                    address,
                    bech32_encoding,
                    TESTNET_INITIAL_BALANCE
                );
                Self::initial_coin(secret, TESTNET_INITIAL_BALANCE, None)
            })
            .collect_vec();

        Self {
            coins: Some(initial_coins),
            ..StateConfig::default()
        }
    }

    #[cfg(feature = "random")]
    pub fn random_testnet() -> Self {
        tracing::info!("Initial Accounts");
        let mut rng = rand::thread_rng();
        let initial_coins = (0..5)
            .map(|_| {
                let secret = SecretKey::random(&mut rng);
                let address = Address::from(*secret.public_key().hash());
                let bech32_data = Bytes32::new(*address).to_base32();
                let bech32_encoding =
                    bech32::encode(FUEL_BECH32_HRP, bech32_data, Bech32m).unwrap();
                tracing::info!(
                    "PrivateKey({:#x}), Address({:#x} [bech32: {}]), Balance({})",
                    secret,
                    address,
                    bech32_encoding,
                    TESTNET_INITIAL_BALANCE
                );
                Self::initial_coin(secret, TESTNET_INITIAL_BALANCE, None)
            })
            .collect_vec();

        Self {
            coins: Some(initial_coins),
            ..StateConfig::default()
        }
    }

    pub fn initial_coin(
        secret: SecretKey,
        amount: u64,
        utxo_id: Option<UtxoId>,
    ) -> CoinConfig {
        let address = Address::from(*secret.public_key().hash());

        CoinConfig {
            tx_id: utxo_id.as_ref().map(|u| *u.tx_id()),
            output_index: utxo_id.as_ref().map(|u| u.output_index()),
            tx_pointer_block_height: None,
            tx_pointer_tx_idx: None,
            maturity: None,
            owner: address,
            amount,
            asset_id: Default::default(),
        }
    }
}

pub trait ChainConfigDb {
    /// Returns *all* unspent coin configs available in the database.
    fn get_coin_config(&self) -> StorageResult<Option<Vec<CoinConfig>>>;
    /// Returns *alive* contract configs available in the database.
    fn get_contract_config(&self) -> StorageResult<Option<Vec<ContractConfig>>>;
    /// Returns *all* unspent message configs available in the database.
    fn get_message_config(&self) -> StorageResult<Option<Vec<MessageConfig>>>;
    /// Returns the last available block height.
    fn get_block_height(&self) -> StorageResult<BlockHeight>;
}

#[cfg(test)]
mod tests {
    use fuel_core_types::{fuel_types::{Bytes32, AssetId}, fuel_tx::{UtxoId, TxPointer}, fuel_vm::Contract, fuel_asm::op, blockchain::primitives::DaBlockHeight};
    use rand::{rngs::StdRng, SeedableRng, Rng, RngCore};

    use crate::{ContractConfig, CoinConfig, MessageConfig};

    use super::StateConfig;

    #[test]
    fn snapshot_simple_contract() {
        let config = test_config_contract(false, false, false, false);
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_simple_contract() {
        let config = test_config_contract(false, false, false, false);
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_contract_with_state() {
        let config = test_config_contract(true, false, false, false);
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_contract_with_state() {
        let config = test_config_contract(true, false, false, false);
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_contract_with_balances() {
        let config = test_config_contract(false, true, false, false);
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_contract_with_balances() {
        let config = test_config_contract(false, true, false, false);
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_contract_with_utxo_id() {
        let config = test_config_contract(false, false, true, false);
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_contract_with_utxoid() {
        let config = test_config_contract(false, false, true, false);
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_contract_with_tx_pointer() {
        let config = test_config_contract(false, false, false, true);
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_contract_with_tx_pointer() {
        let config = test_config_contract(false, false, false, true);
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_simple_coin_state() {
        let config = test_config_coin_state();
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_simple_coin_state() {
        let config = test_config_coin_state();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    #[test]
    fn snapshot_simple_message_state() {
        let config = test_message_config();
        let json = serde_json::to_string_pretty(&config).unwrap();
        insta::assert_snapshot!(json);
    }

    #[test]
    fn can_roundtrip_simple_message_state() {
        let config = test_message_config();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized_config: StateConfig =
            serde_json::from_str(json.as_str()).unwrap();
        assert_eq!(config, deserialized_config);
    }

    fn test_config_contract(
        state: bool,
        balances: bool,
        utxo_id: bool,
        tx_pointer: bool,
    ) -> StateConfig {
        let mut rng = StdRng::seed_from_u64(1);
        let state = if state {
            let test_key: Bytes32 = rng.gen();
            let test_value: Bytes32 = rng.gen();
            Some(vec![(test_key, test_value)])
        } else {
            None
        };
        let balances = if balances {
            let test_asset_id: AssetId = rng.gen();
            let test_balance: u64 = rng.next_u64();
            Some(vec![(test_asset_id, test_balance)])
        } else {
            None
        };
        let utxo_id = if utxo_id {
            Some(UtxoId::new(rng.gen(), rng.gen()))
        } else {
            None
        };
        let tx_pointer = if tx_pointer {
            Some(TxPointer::new(rng.gen(), rng.gen()))
        } else {
            None
        };

        let contract = Contract::from(op::ret(0x10).to_bytes().to_vec());

        StateConfig {
                contracts: Some(vec![ContractConfig {
                    contract_id: Default::default(),
                    code: contract.into(),
                    salt: Default::default(),
                    state,
                    balances,
                    tx_id: utxo_id.map(|utxo_id| *utxo_id.tx_id()),
                    output_index: utxo_id.map(|utxo_id| utxo_id.output_index()),
                    tx_pointer_block_height: tx_pointer.map(|p| p.block_height()),
                    tx_pointer_tx_idx: tx_pointer.map(|p| p.tx_index()),
                }]),
                ..Default::default()
            }
    }

    fn test_config_coin_state() -> StateConfig {
        let mut rng = StdRng::seed_from_u64(1);
        let tx_id: Option<Bytes32> = Some(rng.gen());
        let output_index: Option<u8> = Some(rng.gen());
        let block_created = Some(rng.next_u32().into());
        let block_created_tx_idx = Some(rng.gen());
        let maturity = Some(rng.next_u32().into());
        let owner = rng.gen();
        let amount = rng.gen();
        let asset_id = rng.gen();

        StateConfig {
                coins: Some(vec![CoinConfig {
                    tx_id,
                    output_index,
                    tx_pointer_block_height: block_created,
                    tx_pointer_tx_idx: block_created_tx_idx,
                    maturity,
                    owner,
                    amount,
                    asset_id,
                }]),
                ..Default::default()
            }
    }

    fn test_message_config() -> StateConfig {
        let mut rng = StdRng::seed_from_u64(1);

        StateConfig {
                messages: Some(vec![MessageConfig {
                    sender: rng.gen(),
                    recipient: rng.gen(),
                    nonce: rng.gen(),
                    amount: rng.gen(),
                    data: vec![rng.gen()],
                    da_height: DaBlockHeight(rng.gen()),
                }]),
                ..Default::default()
            }
    }
}
