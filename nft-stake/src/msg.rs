use cosmwasm_std::{Addr, Binary, Uint128};
use schemars::JsonSchema;
use secret_toolkit::{permit::Permit, snip721::ViewerInfo};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    pub entropy: String,
    pub staking_contract: ContractInfo,
    pub reward_contract: RewardsContractInfo,
    pub trait_restriction: Option<String>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ContractInfo {
    pub code_hash: String,
    pub address: Addr,
    pub name: String,
    pub stake_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RewardsContractInfo {
    pub code_hash: String,
    pub address: Addr,
    pub rewards_per_day: Uint128,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Staked {
    pub staked_amount: Uint128,
    pub last_claimed_date: Option<u64>,
    pub last_staked_date: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct History {
    pub amount: Uint128,
    pub date: u64,
    pub action: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    RevokePermit {
        permit_name: String,
    }, 
    Receive {
        sender: Addr,
        from: Addr,
        amount: Uint128,
        msg: Option<Binary>,
    },
    BatchReceiveNft{
        from: Addr, 
        token_ids: Vec<String>,
        msg: Option<Binary>
    },
    WithdrawFunds {},
    WithdrawFundsWithQuantity {
        quantity: Uint128
    },
    WithdrawFundsNoReward {},
    Eject {staker: Addr},
    ClaimRewards {},
    UpdateRewardContract {
        contract: RewardsContractInfo,
    },
    RemoveRewards {},
    SetViewingKey {
        key: String,
    },
    SetActiveState {
        is_active: bool,
    },
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleReceiveMsg { 
    ReceiveRewards {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetStakedInfo {},
    GetMyStakedInfo {
        permit: Permit,
    },
    GetRewardBalance {
        viewer: ViewerInfo,
    },
    GetStakedBalance {
        viewer: ViewerInfo,
    },
    GetNumUserHistory {
        permit: Permit,
    },
    GetUserHistory {
        permit: Permit,
        start_page: u32,
        page_size: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct StakedInfoResponse {
    pub total_staked_amount: Uint128,
    pub staking_contract: ContractInfo,
    pub reward_contract: RewardsContractInfo,
    pub total_rewards: Uint128,
    pub trait_restriction: Option<String>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct MyStakedInfoResponse {
    pub staked: Staked,
    pub estimated_rewards: Uint128,
}
