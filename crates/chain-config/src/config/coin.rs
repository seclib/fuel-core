use crate::GenesisCommitment;
use fuel_core_storage::MerkleRoot;
use fuel_core_types::{
    entities::coins::coin::CompressedCoin,
    fuel_crypto::Hasher,
    fuel_tx::{
        TxPointer,
        UtxoId,
    },
    fuel_types::{
        Address,
        AssetId,
        BlockHeight,
        Bytes32,
    },
};
use rand::Rng;
use serde::{
    Deserialize,
    Serialize,
};
use serde_with::serde_as;

#[serde_as]
#[derive(Default, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CoinConfig {
    #[serde(default = "random_tx_id")]
    pub tx_id: Bytes32,
    #[serde(default = "Default::default")]
    pub output_index: u8,
    /// used if coin is forked from another chain to preserve id & tx_pointer
    pub tx_pointer_block_height: BlockHeight,
    /// used if coin is forked from another chain to preserve id & tx_pointer
    /// The index of the originating tx within `tx_pointer_block_height`
    pub tx_pointer_tx_idx: u16,
    pub maturity: BlockHeight,
    pub owner: Address,
    pub amount: u64,
    pub asset_id: AssetId,
}

fn random_tx_id() -> Bytes32 {
    let mut rng = ::rand::thread_rng();
    rng.gen()
}

impl CoinConfig {
    pub fn utxo_id(&self) -> UtxoId {
        UtxoId::new(self.tx_id, self.output_index)
    }

    pub fn tx_pointer(&self) -> TxPointer {
        TxPointer::new(self.tx_pointer_block_height, self.tx_pointer_tx_idx)
    }
}

#[cfg(all(test, feature = "random", feature = "std"))]
impl crate::Randomize for CoinConfig {
    fn randomize(mut rng: impl ::rand::Rng) -> Self {
        Self {
            tx_id: super::random_bytes_32(&mut rng).into(),
            output_index: rng.gen(),
            tx_pointer_block_height: BlockHeight::new(rng.gen()),
            tx_pointer_tx_idx: rng.gen(),
            maturity: BlockHeight::new(rng.gen()),
            owner: Address::new(super::random_bytes_32(&mut rng)),
            amount: rng.gen(),
            asset_id: AssetId::new(super::random_bytes_32(rng)),
        }
    }
}

impl GenesisCommitment for CompressedCoin {
    fn root(&self) -> anyhow::Result<MerkleRoot> {
        let owner = self.owner();
        let amount = self.amount();
        let asset_id = self.asset_id();
        let maturity = self.maturity();
        let tx_pointer = self.tx_pointer();

        let coin_hash = *Hasher::default()
            .chain(owner)
            .chain(amount.to_be_bytes())
            .chain(asset_id)
            .chain((*maturity).to_be_bytes())
            .chain(tx_pointer.block_height().to_be_bytes())
            .chain(tx_pointer.tx_index().to_be_bytes())
            .finalize();

        Ok(coin_hash)
    }
}
