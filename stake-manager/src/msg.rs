use cosmwasm_std::{Addr, Binary, Uint128};
use schemars::JsonSchema; 
use serde::{Deserialize, Serialize};  
use secret_toolkit::utils::{Query};
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg { 
    pub staking_contracts: Vec<ContractInfo>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ContractInfo {
    pub code_hash: String,
    pub address: Addr,
    pub name: String,
    pub stake_type: String
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct RewardsContractInfo {
    pub code_hash: String,
    pub address: Addr,
    pub rewards_per_day: Uint128,
    pub name: String,
    pub total_rewards: Option<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct StakingWeight {
    pub amount: Uint128,
    pub weight_trait_type: String,
    pub weight_percentage: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    AddContract {
        contract: ContractInfo,
    },
    RemoveContract {
        contract: ContractInfo,
    },
    SetActiveState {
        is_active: bool
    },
}
 
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetContracts {},
    GetContractsWithInfo {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct ContractsWithInfoResponse {
    pub contract_info: ContractInfo,
    pub staked_info: StakedInfoResponse
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct StakedInfoResponse {
    pub total_staked_amount: Uint128,
    pub staking_contract: ContractInfo,
    pub reward_contract: Option<RewardsContractInfo>,
    pub reward_contracts: Option<Vec<RewardsContractInfo>>,
    pub total_rewards: Option<Uint128>,
    pub trait_restriction: Option<String>,
    pub staking_weights: Option<Vec<StakingWeight>>,
    pub is_active: Option<bool>
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StakedInfoQueryMsg{
    GetStakedInfo {}
}

impl Query for StakedInfoQueryMsg{
    const BLOCK_SIZE: usize = 256;
}