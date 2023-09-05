use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::{ContractInfo, History, RewardsContractInfo, Staked, StakingWeight};
use cosmwasm_std::{Addr, CanonicalAddr, Uint128};
use secret_toolkit::{
    snip721::ViewerInfo,
    storage::{AppendStore, Item, Keymap},
};

pub static CONFIG_KEY: &[u8] = b"config";
pub const PREFIX_REVOKED_PERMITS: &str = "revoke";
pub const HISTORY_KEY: &[u8] = b"history";
pub const STAKED_KEY: &[u8] = b"staked";
pub const STAKED_NFTS_KEY: &[u8] = b"staked_nfts";
pub const ADMIN_VIEWING_KEY: &[u8] = b"admin_viewing_key";

pub static CONFIG_ITEM: Item<State> = Item::new(CONFIG_KEY);
pub static HISTORY_STORE: AppendStore<History> = AppendStore::new(HISTORY_KEY);
pub static STAKED_STORE: Keymap<CanonicalAddr, Staked> = Keymap::new(STAKED_KEY);
pub static STAKED_NFTS_STORE: Keymap<CanonicalAddr, Vec<String>> = Keymap::new(STAKED_NFTS_KEY);
pub static ADMIN_VIEWING_KEY_ITEM: Item<ViewerInfo> = Item::new(ADMIN_VIEWING_KEY);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct State {
    pub owner: Addr,
    pub is_active: bool,
    pub staking_contract: ContractInfo,
    pub reward_contracts: Vec<RewardsContractInfo>,
    pub viewing_key: Option<String>,
    pub total_staked_amount: Uint128,
    // pub total_rewards: Uint128,
    pub trait_restriction: Option<String>,
    pub staking_weights: Option<Vec<StakingWeight>>,
}
