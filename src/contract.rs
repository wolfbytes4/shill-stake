use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, HandleReceiveMsg, History, InstantiateMsg, MyStakedInfoResponse, QueryMsg,
    RewardsContractInfo, Staked, StakedInfoResponse,
};
use crate::rand::sha_256;
use crate::state::{
    State, ADMIN_VIEWING_KEY_ITEM, CONFIG_ITEM, HISTORY_STORE, PREFIX_REVOKED_PERMITS, STAKED_STORE,
};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, CanonicalAddr, CosmosMsg, Deps, DepsMut,
    Env, MessageInfo, Response, StdError, StdResult, Uint128,
};
use secret_toolkit::{
    permit::{validate, Permit, RevokedPermits},
    snip20::{balance_query, set_viewing_key_msg, transfer_msg, Balance},
    snip721::ViewerInfo,
};

pub const BLOCK_SIZE: usize = 256;
///  Add function to get balance

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, StdError> {
    let prng_seed: Vec<u8> = sha_256(base64::encode(msg.entropy).as_bytes()).to_vec();
    let viewing_key = base64::encode(&prng_seed);

    // create initial state
    let state = State {
        viewing_key: Some(viewing_key),
        owner: info.sender.clone(),
        staking_contract: msg.staking_contract,
        reward_contract: msg.reward_contract,
        total_staked_amount: Uint128::from(0u128),
        total_rewards: Uint128::from(0u128),
        is_active: true,
    };

    //Save Contract state
    CONFIG_ITEM.save(deps.storage, &state)?;

    let mut response_msgs: Vec<CosmosMsg> = Vec::new();

    deps.api
        .debug(&format!("Contract was initialized by {}", info.sender));

    let vk = state.viewing_key.unwrap();

    response_msgs.push(set_viewing_key_msg(
        vk.to_string(),
        None,
        BLOCK_SIZE,
        state.staking_contract.code_hash,
        state.staking_contract.address.to_string(),
    )?);

    response_msgs.push(set_viewing_key_msg(
        vk.to_string(),
        None,
        BLOCK_SIZE,
        state.reward_contract.code_hash.to_string(),
        state.reward_contract.address.to_string(),
    )?);

    Ok(Response::new().add_messages(response_msgs))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RevokePermit { permit_name } => {
            try_revoke_permit(deps, &info.sender, &permit_name)
        }
        ExecuteMsg::UpdateRewardContract { contract } => {
            try_update_reward_contract(deps, &info.sender, contract)
        }
        ExecuteMsg::RemoveRewards {} => try_remove_rewards(deps, &info.sender),
        ExecuteMsg::Receive {
            sender,
            from,
            amount,
            msg,
        } => receive(deps, _env, &info.sender, &sender, &from, amount, msg),
        ExecuteMsg::WithdrawFunds {} => try_withdraw(deps, _env, &info.sender),
        ExecuteMsg::ClaimRewards {} => try_claim_rewards(deps, _env, &info.sender),
        ExecuteMsg::SetViewingKey { key } => try_set_viewing_key(deps, _env, &info.sender, key),
        ExecuteMsg::SetActiveState { is_active } => {
            try_set_active_state(deps, _env, &info.sender, is_active)
        }
    }
}

fn receive(
    deps: DepsMut,
    _env: Env,
    info_sender: &Addr, //snip contract
    sender: &Addr,      //for snip 20 sender and from are the same. Wth??
    from: &Addr,        //user
    amount: Uint128,
    msg: Option<Binary>,
) -> Result<Response, ContractError> {
    deps.api.debug(&format!("Receive received"));

    let mut response_msgs: Vec<CosmosMsg> = Vec::new();
    let mut state = CONFIG_ITEM.load(deps.storage)?;
    if !state.is_active {
        return Err(ContractError::CustomError {
            val: "You cannot perform this action right now".to_string(),
        });
    }
    if let Some(bin_msg) = msg {
        match from_binary(&bin_msg)? {
            //STAKE MESSAGE
            HandleReceiveMsg::ReceiveStake {} => {
                let history_store = HISTORY_STORE.add_suffix(from.to_string().as_bytes());

                if info_sender != &state.staking_contract.address {
                    return Err(ContractError::CustomError {
                        val: info_sender.to_string()
                            + &" Address is not correct snip contract".to_string(),
                    });
                }
                let current_time = _env.block.time.seconds();
                let mut staked = STAKED_STORE
                    .get(
                        deps.storage,
                        &deps.api.addr_canonicalize(&from.to_string())?,
                    )
                    .unwrap_or(Staked {
                        last_claimed_date: None,
                        staked_amount: Uint128::from(0u128),
                        last_staked_date: Some(current_time),
                    });
                let date_to_compare = if staked.last_claimed_date.is_some() {
                    staked.last_claimed_date.unwrap()
                } else {
                    staked.last_staked_date.unwrap()
                };
                if current_time > date_to_compare {
                    //claim rewards
                    staked.last_claimed_date = Some(current_time);
                    let user_reward_percentage = staked.staked_amount / state.total_staked_amount;
                    let elapsed_seconds = current_time - date_to_compare;
                    let rewards_per_second = state.reward_contract.rewards_per_day
                        / Uint128::from(24u64 * 60u64 * 60u64);
                    let rewards_to_claim = user_reward_percentage
                        * rewards_per_second
                        * Uint128::from(elapsed_seconds);
                    let claim_history: History = {
                        History {
                            amount: rewards_to_claim,
                            date: current_time,
                            action: "claim".to_string(),
                        }
                    };

                    history_store.push(deps.storage, &claim_history)?;
                    response_msgs.push(transfer_msg(
                        from.to_string(),
                        rewards_to_claim,
                        None,
                        None,
                        BLOCK_SIZE,
                        state.reward_contract.code_hash.to_string(),
                        state.reward_contract.address.to_string(),
                    )?);
                }

                state.total_staked_amount += amount;
                staked.staked_amount += amount;
                staked.last_staked_date = Some(current_time);
                CONFIG_ITEM.save(deps.storage, &state)?;
                STAKED_STORE.insert(
                    deps.storage,
                    &deps.api.addr_canonicalize(&from.to_string())?,
                    &staked,
                )?;

                let stake_history: History = {
                    History {
                        amount: amount,
                        date: current_time,
                        action: "stake".to_string(),
                    }
                };

                history_store.push(deps.storage, &stake_history)?;
            }
            HandleReceiveMsg::ReceiveRewards {} => {
                if info_sender != &state.reward_contract.address {
                    return Err(ContractError::CustomError {
                        val: info_sender.to_string()
                            + &" Address is not correct reward snip contract".to_string(),
                    });
                }
                state.total_rewards += amount;

                CONFIG_ITEM.save(deps.storage, &state)?;
            }
        }
    } else {
        return Err(ContractError::CustomError {
            val: "data should be given".to_string(),
        });
    }

    Ok(Response::new().add_messages(response_msgs))
}

fn try_withdraw(deps: DepsMut, _env: Env, info_sender: &Addr) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;
    let history_store = HISTORY_STORE.add_suffix(info_sender.to_string().as_bytes());
    let current_time = _env.block.time.seconds();
    let staked = STAKED_STORE
        .get(
            deps.storage,
            &deps.api.addr_canonicalize(&info_sender.to_string())?,
        )
        .ok_or_else(|| StdError::generic_err("You aren't staked"))?;

    if staked.staked_amount == Uint128::from(0u128) {
        return Err(ContractError::CustomError {
            val: "There is nothing to claim".to_string(),
        });
    }

    let mut response_msgs: Vec<CosmosMsg> = Vec::new();
    response_msgs.push(transfer_msg(
        info_sender.to_string(),
        staked.staked_amount.clone(),
        None,
        None,
        BLOCK_SIZE,
        state.staking_contract.code_hash.to_string(),
        state.staking_contract.address.to_string(),
    )?);
    state.total_staked_amount -= staked.staked_amount;

    CONFIG_ITEM.save(deps.storage, &state)?;
    STAKED_STORE.remove(
        deps.storage,
        &deps.api.addr_canonicalize(&info_sender.to_string())?,
    )?;

    let stake_history: History = {
        History {
            amount: staked.staked_amount,
            date: current_time,
            action: "withdraw".to_string(),
        }
    };

    history_store.push(deps.storage, &stake_history)?;
    Ok(Response::new().add_messages(response_msgs))
}

fn try_claim_rewards(
    deps: DepsMut,
    _env: Env,
    info_sender: &Addr,
) -> Result<Response, ContractError> {
    let state = CONFIG_ITEM.load(deps.storage)?;
    let history_store = HISTORY_STORE.add_suffix(info_sender.to_string().as_bytes());
    let current_time = _env.block.time.seconds();
    let mut staked = STAKED_STORE
        .get(
            deps.storage,
            &deps.api.addr_canonicalize(&info_sender.to_string())?,
        )
        .ok_or_else(|| StdError::generic_err("You aren't staked"))?;

    if staked.staked_amount == Uint128::from(0u128) {
        return Err(ContractError::CustomError {
            val: "There is nothing to claim".to_string(),
        });
    }

    let mut response_msgs: Vec<CosmosMsg> = Vec::new();
    let date_to_compare = if staked.last_claimed_date.is_some() {
        staked.last_claimed_date.unwrap()
    } else {
        staked.last_staked_date.unwrap()
    };
    if current_time > date_to_compare {
        //claim rewards
        staked.last_claimed_date = Some(current_time);
        let user_reward_percentage = staked.staked_amount / state.total_staked_amount;
        let elapsed_seconds = current_time - date_to_compare;

        let rewards_per_second =
            state.reward_contract.rewards_per_day / Uint128::from(24u64 * 60u64 * 60u64);
        let rewards_to_claim =
            user_reward_percentage * rewards_per_second * Uint128::from(elapsed_seconds);
        if state.total_rewards < rewards_to_claim {
            return Err(ContractError::CustomError {
                val: "Error trying to claim rewards".to_string(),
            });
        }
        let claim_history: History = {
            History {
                amount: rewards_to_claim,
                date: current_time,
                action: "claim".to_string(),
            }
        };

        history_store.push(deps.storage, &claim_history)?;
        response_msgs.push(transfer_msg(
            info_sender.to_string(),
            rewards_to_claim,
            None,
            None,
            BLOCK_SIZE,
            state.reward_contract.code_hash.to_string(),
            state.reward_contract.address.to_string(),
        )?);
    } else {
        //this technically should never happen
        return Err(ContractError::CustomError {
            val: "Not allowed to claim yet".to_string(),
        });
    }

    Ok(Response::new().add_messages(response_msgs))
}

fn try_revoke_permit(
    deps: DepsMut,
    sender: &Addr,
    permit_name: &str,
) -> Result<Response, ContractError> {
    RevokedPermits::revoke_permit(
        deps.storage,
        PREFIX_REVOKED_PERMITS,
        &sender.to_string(),
        permit_name,
    );

    Ok(Response::default())
}

fn try_update_reward_contract(
    deps: DepsMut,
    sender: &Addr,
    contract: RewardsContractInfo,
) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;

    if sender.clone() != state.owner {
        return Err(ContractError::CustomError {
            val: "You don't have the permissions to execute this command".to_string(),
        });
    }

    if state.total_rewards != Uint128::from(0u128) {
        return Err(ContractError::CustomError {
            val: "Clear out rewards first before updating".to_string(),
        });
    }

    state.reward_contract = contract;
    CONFIG_ITEM.save(deps.storage, &state)?;
    Ok(Response::new().add_message(set_viewing_key_msg(
        state.viewing_key.unwrap().to_string(),
        None,
        BLOCK_SIZE,
        state.reward_contract.code_hash,
        state.reward_contract.address.to_string(),
    )?))
}

fn try_remove_rewards(deps: DepsMut, sender: &Addr) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;

    if sender.clone() != state.owner {
        return Err(ContractError::CustomError {
            val: "You don't have the permissions to execute this command".to_string(),
        });
    }

    let cosmos_msg = transfer_msg(
        sender.to_string(),
        state.total_rewards.clone(),
        None,
        None,
        BLOCK_SIZE,
        state.reward_contract.code_hash.to_string(),
        state.reward_contract.address.to_string(),
    )?;

    state.total_rewards = Uint128::from(0u128);
    CONFIG_ITEM.save(deps.storage, &state)?;
    Ok(Response::new().add_message(cosmos_msg))
}

pub fn try_set_viewing_key(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    key: String,
) -> Result<Response, ContractError> {
    let state = CONFIG_ITEM.load(deps.storage)?;
    let prng_seed: Vec<u8> = sha_256(base64::encode(key).as_bytes()).to_vec();
    let viewing_key = base64::encode(&prng_seed);

    let vk: ViewerInfo = {
        ViewerInfo {
            address: sender.to_string(),
            viewing_key: viewing_key,
        }
    };

    if sender.clone() == state.owner {
        ADMIN_VIEWING_KEY_ITEM.save(deps.storage, &vk)?;
    } else {
        return Err(ContractError::CustomError {
            val: "You don't have the permissions to execute this command".to_string(),
        });
    }
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
        QueryMsg::GetStakedInfo {} => to_binary(&query_staked(deps)?),
        QueryMsg::GetMyStakedInfo { permit } => to_binary(&query_my_staked(deps, _env, permit)?),
        QueryMsg::GetNumUserHistory { permit } => {
            to_binary(&query_num_user_history(deps, _env, permit)?)
        }
        QueryMsg::GetUserHistory {
            permit,
            start_page,
            page_size,
        } => to_binary(&query_user_history(
            deps, _env, permit, start_page, page_size,
        )?),
        QueryMsg::GetRewardBalance { viewer } => {
            to_binary(&query_reward_balance(deps, _env, viewer)?)
        }
        QueryMsg::GetStakedBalance { viewer } => {
            to_binary(&query_staked_balance(deps, _env, viewer)?)
        }
    }
}

fn query_staked(deps: Deps) -> StdResult<StakedInfoResponse> {
    let state = CONFIG_ITEM.load(deps.storage)?;
    Ok(StakedInfoResponse {
        total_staked_amount: state.total_staked_amount,
        total_rewards: state.total_rewards,
        staking_contract: state.staking_contract,
        reward_contract: state.reward_contract,
    })
}

fn query_my_staked(deps: Deps, env: Env, permit: Permit) -> StdResult<MyStakedInfoResponse> {
    let user_raw = get_querier(deps, permit, env.contract.address)?;
    let state = CONFIG_ITEM.load(deps.storage)?;

    let staked = STAKED_STORE.get(deps.storage, &user_raw).unwrap_or(Staked {
        last_claimed_date: None,
        staked_amount: Uint128::from(0u128),
        last_staked_date: None,
    });
    let estimated_rewards = Uint128::from(0u128);
    if staked.staked_amount > Uint128::from(0u128) {
        let current_time = env.block.time.seconds();
        let date = if staked.last_claimed_date.is_some() {
            staked.last_claimed_date.unwrap()
        } else {
            staked.last_staked_date.unwrap()
        };

        let user_reward_percentage = staked.staked_amount / state.total_staked_amount;
        let elapsed_seconds = current_time - date;
        let rewards_per_second =
            state.reward_contract.rewards_per_day / Uint128::from(24u64 * 60u64 * 60u64);
        let rewards_to_claim =
            user_reward_percentage * rewards_per_second * Uint128::from(elapsed_seconds);
    }

    Ok(MyStakedInfoResponse {
        staked: staked,
        estimated_rewards: estimated_rewards,
    })
}

fn query_num_user_history(deps: Deps, env: Env, permit: Permit) -> StdResult<u32> {
    let user_raw = get_querier(deps, permit, env.contract.address)?;
    let history_store = HISTORY_STORE.add_suffix(&user_raw);
    let num = history_store.get_len(deps.storage)?;
    Ok(num)
}

fn query_user_history(
    deps: Deps,
    env: Env,
    permit: Permit,
    start_page: u32,
    page_size: u32,
) -> StdResult<Vec<History>> {
    let user_raw = get_querier(deps, permit, env.contract.address)?;
    let history_store = HISTORY_STORE.add_suffix(&user_raw);
    let history = history_store.paging(deps.storage, start_page, page_size)?;
    Ok(history)
}

fn query_reward_balance(deps: Deps, env: Env, viewer: ViewerInfo) -> StdResult<Balance> {
    check_admin_key(deps, viewer)?;
    let state = CONFIG_ITEM.load(deps.storage)?;
    let balance = balance_query(
        deps.querier,
        env.contract.address.to_string(),
        state.viewing_key.unwrap(),
        BLOCK_SIZE,
        state.reward_contract.code_hash,
        state.reward_contract.address.to_string(),
    );
    Ok(balance.unwrap())
}

fn query_staked_balance(deps: Deps, env: Env, viewer: ViewerInfo) -> StdResult<Balance> {
    check_admin_key(deps, viewer)?;
    let state = CONFIG_ITEM.load(deps.storage)?;
    let balance = balance_query(
        deps.querier,
        env.contract.address.to_string(),
        state.viewing_key.unwrap(),
        BLOCK_SIZE,
        state.staking_contract.code_hash,
        state.staking_contract.address.to_string(),
    );
    Ok(balance.unwrap())
}

fn check_admin_key(deps: Deps, viewer: ViewerInfo) -> StdResult<()> {
    let admin_viewing_key = ADMIN_VIEWING_KEY_ITEM.load(deps.storage)?;
    let prng_seed: Vec<u8> = sha_256(base64::encode(viewer.viewing_key).as_bytes()).to_vec();
    let vk = base64::encode(&prng_seed);

    if vk != admin_viewing_key.viewing_key || viewer.address != admin_viewing_key.address {
        return Err(StdError::generic_err(
            "Wrong viewing key for this address or viewing key not set",
        ));
    }

    return Ok(());
}

fn get_querier(deps: Deps, permit: Permit, contract_address: Addr) -> StdResult<CanonicalAddr> {
    if let pmt = permit {
        let querier = deps.api.addr_canonicalize(&validate(
            deps,
            PREFIX_REVOKED_PERMITS,
            &pmt,
            contract_address.to_string(),
            None,
        )?)?;
        if !pmt.check_permission(&secret_toolkit::permit::TokenPermissions::Owner) {
            return Err(StdError::generic_err(format!(
                "Owner permission is required for history queries, got permissions {:?}",
                pmt.params.permissions
            )));
        }
        return Ok(querier);
    }
    return Err(StdError::generic_err("Unauthorized"));
}
