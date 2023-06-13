use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, InstantiateMsg, QueryMsg,
    RewardsContractInfo, ContractInfo, StakedInfoResponse, StakedInfoQueryMsg, ContractsWithInfoResponse
}; 
use crate::state::{
    State, CONFIG_ITEM
};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, CanonicalAddr, CosmosMsg, Deps, DepsMut,
    Env, MessageInfo, Response, StdError, StdResult, Uint128,
}; 
use secret_toolkit::utils::{Query};

pub const BLOCK_SIZE: usize = 256;
///  Add function to get balance

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, StdError> { 
    // create initial state
    let state = State { 
        owner: info.sender.clone(),
        staking_contracts: msg.staking_contracts, 
        is_active: true,
    };

    //Save Contract state
    CONFIG_ITEM.save(deps.storage, &state)?;
    Ok(Response::default())
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg { 
        ExecuteMsg::AddContract { contract } => {
            try_add_contract(deps, &info.sender, contract)
        } 
        ExecuteMsg::RemoveContract { contract } => {
            try_remove_contract(deps, &info.sender, contract)
        } 
        ExecuteMsg::SetActiveState { is_active } => {
            try_set_active_state(deps, _env, &info.sender, is_active)
        }
    }
}

fn try_add_contract(
    deps: DepsMut,
    sender: &Addr,
    contract: ContractInfo,
) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;

    if sender.clone() != state.owner {
        return Err(ContractError::CustomError {
            val: "You don't have the permissions to execute this command".to_string(),
        });
    } 
    
    let position = state.staking_contracts.iter().position(|x| x.address == contract.address);
    if position.is_some(){
        return Err(ContractError::CustomError {val: "Contract already exists".to_string()});  
    }

    state.staking_contracts.push(contract);
    CONFIG_ITEM.save(deps.storage, &state)?;
    Ok(Response::default())
}

fn try_remove_contract(
    deps: DepsMut,
    sender: &Addr,
    contract: ContractInfo,
) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;

    if sender.clone() != state.owner {
        return Err(ContractError::CustomError {
            val: "You don't have the permissions to execute this command".to_string(),
        });
    } 
    let position = state.staking_contracts.iter().position(|x| x.address == contract.address);
         
    if position.is_none(){
        return Err(ContractError::CustomError {val: "Contract doesn't exist".to_string()});  
    }
    else{ 
        state.staking_contracts.remove(position.unwrap());
    } 
    CONFIG_ITEM.save(deps.storage, &state)?;
    Ok(Response::default())
}
  

pub fn try_set_active_state(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    is_active: bool,
) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;

    state.is_active = is_active;

    CONFIG_ITEM.save(deps.storage, &state)?;

    Ok(Response::default())
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetContracts {} => to_binary(&query_contracts(deps)?),
        QueryMsg::GetContractsWithInfo {} => to_binary(&query_contracts_info(deps)?),
    }
}

fn query_contracts(deps: Deps) -> StdResult<Vec<ContractInfo>> {
    let state = CONFIG_ITEM.load(deps.storage)?;
    Ok(state.staking_contracts)
}

fn query_contracts_info(deps: Deps) -> StdResult<Vec<ContractsWithInfoResponse>> {
    let state = CONFIG_ITEM.load(deps.storage)?;
    let mut response: Vec<ContractsWithInfoResponse> = Vec::new();

    let get_staked_info  = StakedInfoQueryMsg::GetStakedInfo {};
    for contract in state.staking_contracts.iter() {
        let staked_info_response: StdResult<StakedInfoResponse> = get_staked_info.query(
            deps.querier,
            contract.code_hash.to_string(),
            contract.address.to_string()
        );
        let info: ContractsWithInfoResponse = {ContractsWithInfoResponse{
            contract_info: contract.clone(),
            staked_info: staked_info_response.unwrap()
        }};
        response.push(info);
    }

    Ok(response)
}

 
 
 
 


