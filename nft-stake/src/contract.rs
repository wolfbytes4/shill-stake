use crate::error::ContractError;
use crate::msg::{
    EstimatedReward, ExecuteMsg, HandleReceiveMsg, History, InstantiateMsg, MyStakedInfoResponse,
    QueryMsg, RewardsContractInfo, Staked, StakedInfoResponse, StakingWeight, UserStakingWeight,
};
use crate::rand::sha_256;
use crate::state::{
    State, ADMIN_VIEWING_KEY_ITEM, CONFIG_ITEM, HISTORY_STORE, PREFIX_REVOKED_PERMITS,
    STAKED_NFTS_STORE, STAKED_STORE,
};
use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, CanonicalAddr, CosmosMsg, Decimal, Deps,
    DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128,
};
use secret_toolkit::{
    permit::{validate, Permit, RevokedPermits},
    snip20::{balance_query, set_viewing_key_msg, transfer_msg, Balance},
    snip721::{
        batch_transfer_nft_msg, nft_dossier_query, register_receive_nft_msg, NftDossier, Transfer,
        ViewerInfo,
    },
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
        reward_contracts: msg.reward_contracts,
        is_active: true,
        trait_restriction: msg.trait_restriction,
        staking_weights: msg.staking_weights,
        total_staked_amount: Uint128::from(0u128),
    };

    //Save Contract state
    CONFIG_ITEM.save(deps.storage, &state)?;

    let mut response_msgs: Vec<CosmosMsg> = Vec::new();

    deps.api
        .debug(&format!("Contract was initialized by {}", info.sender));

    let vk = state.viewing_key.unwrap();

    response_msgs.push(register_receive_nft_msg(
        _env.contract.code_hash,
        Some(true),
        None,
        BLOCK_SIZE,
        state.staking_contract.code_hash.clone(),
        state.staking_contract.address.to_string(),
    )?);

    response_msgs.push(set_viewing_key_msg(
        vk.to_string(),
        None,
        BLOCK_SIZE,
        state.staking_contract.code_hash,
        state.staking_contract.address.to_string(),
    )?);

    for reward_contract in state.reward_contracts.iter() {
        response_msgs.push(set_viewing_key_msg(
            vk.to_string(),
            None,
            BLOCK_SIZE,
            reward_contract.code_hash.to_string(),
            reward_contract.address.to_string(),
        )?);
    }

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
        ExecuteMsg::UpdateRewardContract { contracts } => {
            try_update_reward_contract(deps, &info.sender, contracts)
        }
        ExecuteMsg::RemoveRewards {} => try_remove_rewards(deps, &info.sender),
        ExecuteMsg::BatchReceiveNft {
            from,
            token_ids,
            msg,
        } => try_batch_receive(deps, _env, &info.sender, &from, token_ids, msg),
        ExecuteMsg::Receive {
            sender,
            from,
            amount,
            msg,
        } => receive(deps, _env, &info.sender, &sender, &from, amount, msg),
        ExecuteMsg::WithdrawFunds {} => try_withdraw(deps, _env, &info.sender),
        ExecuteMsg::WithdrawFundsWithQuantity { quantity } => {
            try_withdraw_with_quantity(deps, _env, &info.sender, quantity)
        }
        ExecuteMsg::WithdrawFundsNoReward {} => try_withdraw_no_reward(deps, _env, &info.sender),
        ExecuteMsg::ClaimRewards {} => try_claim_rewards(deps, _env, &info.sender),
        ExecuteMsg::SetViewingKey { key } => try_set_viewing_key(deps, _env, &info.sender, key),
        ExecuteMsg::SetActiveState { is_active } => {
            try_set_active_state(deps, _env, &info.sender, is_active)
        }
        ExecuteMsg::Eject { staker } => try_eject(deps, _env, &info.sender, &staker),
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

    if let Some(bin_msg) = msg {
        match from_binary(&bin_msg)? {
            HandleReceiveMsg::ReceiveRewards {} => {
                let reward_contract_index = state
                    .reward_contracts
                    .iter()
                    .position(|x| x.address == info_sender.to_string());
                if reward_contract_index.is_none() {
                    return Err(ContractError::CustomError {
                        val: info_sender.to_string()
                            + &" Address is not correct reward snip contract".to_string(),
                    });
                }
                let reward_contract = &mut state.reward_contracts[reward_contract_index.unwrap()];
                reward_contract.total_rewards += amount;

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
fn try_batch_receive(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    from: &Addr,
    token_ids: Vec<String>,
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
    let history_store = HISTORY_STORE.add_suffix(from.to_string().as_bytes());

    if sender != &state.staking_contract.address {
        return Err(ContractError::CustomError {
            val: sender.to_string() + &" Address is not correct snip contract".to_string(),
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
            staking_weights: Some(Vec::new()),
        });
    let mut staked_nfts = STAKED_NFTS_STORE
        .get(
            deps.storage,
            &deps.api.addr_canonicalize(&from.to_string())?,
        )
        .unwrap_or(Vec::new());

    for id in token_ids.iter() {
        if state.trait_restriction.is_some() || state.staking_weights.is_some() {
            let meta: NftDossier = nft_dossier_query(
                deps.querier,
                id.to_string(),
                None,
                None,
                BLOCK_SIZE,
                state.staking_contract.code_hash.clone(),
                state.staking_contract.address.to_string(),
            )?;

            if state.trait_restriction.is_some() {
                let trait_to_check = state.trait_restriction.as_ref().unwrap();
                let restricted_trait = meta
                    .public_metadata
                    .as_ref()
                    .unwrap()
                    .extension
                    .as_ref()
                    .unwrap()
                    .attributes
                    .as_ref()
                    .unwrap()
                    .iter()
                    .find(|&x| x.trait_type == Some(trait_to_check.to_string()));
                if restricted_trait.is_none() {
                    return Err(ContractError::CustomError {
                        val: "This NFT does not meet the requirements".to_string(),
                    });
                }
            }

            if state.staking_weights.is_some() {
                let mut new_user_weights: Vec<UserStakingWeight> = Vec::new();
                for weight in state.staking_weights.as_mut().unwrap().iter_mut() {
                    let weight_trait_to_check = &weight.weight_trait_type;
                    let weight_trait = meta
                        .public_metadata
                        .as_ref()
                        .unwrap()
                        .extension
                        .as_ref()
                        .unwrap()
                        .attributes
                        .as_ref()
                        .unwrap()
                        .iter()
                        .find(|&x| x.trait_type == Some(weight_trait_to_check.to_string()));
                    if weight_trait.is_none() {
                        return Err(ContractError::CustomError {
                            val: "This NFT does not meet the weight requirements".to_string(),
                        });
                    }
                    //add to overall pool weight amount
                    let weight_amount =
                        Uint128::from(weight_trait.unwrap().value.parse::<u32>().unwrap());
                    weight.amount += weight_amount;
                    let user_staking_weight = staked
                        .staking_weights
                        .as_ref()
                        .unwrap()
                        .iter()
                        .find(|x| x.weight_trait_type == weight.weight_trait_type.to_string());
                    let mut weight_update = UserStakingWeight {
                        amount: Uint128::from(0u128),
                        weight_trait_type: weight.weight_trait_type.to_string(),
                    };

                    if user_staking_weight.is_some() {
                        weight_update.amount = user_staking_weight.unwrap().amount;
                    }

                    weight_update.amount += weight_amount;
                    new_user_weights.push(weight_update);
                }
                staked.staking_weights = Some(new_user_weights);
            }
        }
        staked_nfts.push(id.to_string());
    }

    let current_time = _env.block.time.seconds();
    let rewards_to_claim = get_estimated_rewards(&staked, &current_time, &state)?;
    for rewards in rewards_to_claim.iter() {
        let reward_contract_index = state
            .reward_contracts
            .iter()
            .position(|x| x.name == rewards.reward_contract_name.to_string());
        let reward_contract = &mut state.reward_contracts[reward_contract_index.unwrap()];

        if rewards.estimated_rewards > Uint128::from(0u128)
            && rewards.estimated_rewards < reward_contract.total_rewards
        {
            //claim rewards
            staked.last_claimed_date = Some(current_time);
            let claim_history: History = {
                History {
                    amount: rewards.estimated_rewards,
                    date: current_time,
                    action: "claim".to_string(),
                }
            };

            history_store.push(deps.storage, &claim_history)?;
            response_msgs.push(transfer_msg(
                from.to_string(),
                rewards.estimated_rewards,
                None,
                None,
                BLOCK_SIZE,
                reward_contract.code_hash.to_string(),
                reward_contract.address.to_string(),
            )?);
            reward_contract.total_rewards -= rewards.estimated_rewards;
        }
    }

    state.total_staked_amount += Uint128::from(token_ids.len() as u128);
    staked.staked_amount += Uint128::from(token_ids.len() as u128);
    staked.last_staked_date = Some(current_time);
    CONFIG_ITEM.save(deps.storage, &state)?;
    STAKED_STORE.insert(
        deps.storage,
        &deps.api.addr_canonicalize(&from.to_string())?,
        &staked,
    )?;

    STAKED_NFTS_STORE.insert(
        deps.storage,
        &deps.api.addr_canonicalize(&from.to_string())?,
        &staked_nfts,
    )?;

    let stake_history: History = {
        History {
            amount: Uint128::from(token_ids.len() as u128),
            date: current_time,
            action: "stake".to_string(),
        }
    };

    history_store.push(deps.storage, &stake_history)?;

    Ok(Response::new().add_messages(response_msgs))
}

fn try_withdraw_with_quantity(
    deps: DepsMut,
    _env: Env,
    info_sender: &Addr,
    quantity: Uint128,
) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;
    let history_store = HISTORY_STORE.add_suffix(info_sender.to_string().as_bytes());
    let current_time = _env.block.time.seconds();
    let staked = STAKED_STORE
        .get(
            deps.storage,
            &deps.api.addr_canonicalize(&info_sender.to_string())?,
        )
        .ok_or_else(|| StdError::generic_err("You aren't staked"))?;

    let mut staked_nfts = STAKED_NFTS_STORE
        .get(
            deps.storage,
            &deps.api.addr_canonicalize(&info_sender.to_string())?,
        )
        .ok_or_else(|| StdError::generic_err("NFTs aren't staked"))?;

    if staked.staked_amount == Uint128::from(0u128) {
        return Err(ContractError::CustomError {
            val: "There is nothing to withdraw".to_string(),
        });
    }

    if staked_nfts.len() == 0 {
        return Err(ContractError::CustomError {
            val: "There are no NFTs to withdraw".to_string(),
        });
    }

    if Uint128::from(staked_nfts.len() as u128) < quantity {
        return Err(ContractError::CustomError {
            val: "You are trying to withdraw more than is staked".to_string(),
        });
    }

    if state.staking_weights.is_some() {
        return Err(ContractError::CustomError {
            val: "Not supported for collections that have weighted staking".to_string(),
        });
    }

    let mut response_msgs: Vec<CosmosMsg> = Vec::new();
    let rewards_to_claim = get_estimated_rewards(&staked, &current_time, &state)?;

    for rewards in rewards_to_claim.iter() {
        let reward_contract_index = state
            .reward_contracts
            .iter()
            .position(|x| x.name == rewards.reward_contract_name.to_string());
        let reward_contract = &mut state.reward_contracts[reward_contract_index.unwrap()];

        if rewards.estimated_rewards > Uint128::from(0u128)
            && rewards.estimated_rewards < reward_contract.total_rewards
        {
            //claim rewards
            let claim_history: History = {
                History {
                    amount: rewards.estimated_rewards,
                    date: current_time,
                    action: "claim".to_string(),
                }
            };

            history_store.push(deps.storage, &claim_history)?;
            response_msgs.push(transfer_msg(
                info_sender.to_string(),
                rewards.estimated_rewards,
                None,
                None,
                BLOCK_SIZE,
                reward_contract.code_hash.to_string(),
                reward_contract.address.to_string(),
            )?);
            reward_contract.total_rewards -= rewards.estimated_rewards;
        }
    }
    //QUANTITY LOGIC
    let staked_nfts_leftover = staked_nfts.split_off(quantity.u128() as usize);
    let staked_nfts_leftover_len = Uint128::from(staked_nfts_leftover.len() as u128);
    state.total_staked_amount -= quantity;

    let mut transfers: Vec<Transfer> = Vec::new();
    transfers.push(Transfer {
        recipient: info_sender.to_string(),
        token_ids: staked_nfts,
        memo: None,
    });

    let cosmos_batch_msg = batch_transfer_nft_msg(
        transfers,
        None,
        BLOCK_SIZE,
        state.staking_contract.code_hash.clone(),
        state.staking_contract.address.to_string(),
    )?;
    response_msgs.push(cosmos_batch_msg);
    CONFIG_ITEM.save(deps.storage, &state)?;
    STAKED_STORE.insert(
        deps.storage,
        &deps.api.addr_canonicalize(&info_sender.to_string())?,
        &Staked {
            last_claimed_date: None,
            staked_amount: Uint128::from(staked_nfts_leftover.len() as u128),
            last_staked_date: if staked_nfts_leftover.len() > 0 {
                Some(current_time)
            } else {
                None
            },
            staking_weights: Some(Vec::new()),
        },
    )?;

    STAKED_NFTS_STORE.insert(
        deps.storage,
        &deps.api.addr_canonicalize(&info_sender.to_string())?,
        &staked_nfts_leftover,
    )?;

    let stake_history: History = {
        History {
            amount: staked_nfts_leftover_len,
            date: current_time,
            action: "withdraw".to_string(),
        }
    };

    history_store.push(deps.storage, &stake_history)?;
    Ok(Response::new().add_messages(response_msgs))
}

fn try_withdraw(deps: DepsMut, _env: Env, info_sender: &Addr) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;
    let history_store = HISTORY_STORE.add_suffix(info_sender.to_string().as_bytes());
    let current_time = _env.block.time.seconds();
    let mut response_msgs: Vec<CosmosMsg> = Vec::new();

    let staked = STAKED_STORE
        .get(
            deps.storage,
            &deps.api.addr_canonicalize(&info_sender.to_string())?,
        )
        .ok_or_else(|| StdError::generic_err("You aren't staked"))?;

    let staked_nfts = STAKED_NFTS_STORE
        .get(
            deps.storage,
            &deps.api.addr_canonicalize(&info_sender.to_string())?,
        )
        .ok_or_else(|| StdError::generic_err("NFTs aren't staked"))?;

    if staked.staked_amount == Uint128::from(0u128) {
        return Err(ContractError::CustomError {
            val: "There is nothing to withdraw".to_string(),
        });
    }

    if staked_nfts.len() == 0 {
        return Err(ContractError::CustomError {
            val: "There are no NFTs to withdraw".to_string(),
        });
    }

    let rewards_to_claim = get_estimated_rewards(&staked, &current_time, &state)?;

    for rewards in rewards_to_claim.iter() {
        let reward_contract_index = state
            .reward_contracts
            .iter()
            .position(|x| x.name == rewards.reward_contract_name.to_string());
        let reward_contract = &mut state.reward_contracts[reward_contract_index.unwrap()];

        if state.staking_weights.is_some() {
            for weight in state.staking_weights.as_mut().unwrap().iter_mut() {
                let user_staking_weight = staked
                    .staking_weights
                    .as_ref()
                    .unwrap()
                    .iter()
                    .find(|x| x.weight_trait_type == weight.weight_trait_type);
                if user_staking_weight.is_none() {
                    return Err(ContractError::CustomError {
                        val: "Can't find matching user weight".to_string(),
                    });
                }

                weight.amount -= user_staking_weight.unwrap().amount;
            }
        }

        if rewards.estimated_rewards > Uint128::from(0u128)
            && rewards.estimated_rewards < reward_contract.total_rewards
        {
            //claim rewards
            let claim_history: History = {
                History {
                    amount: rewards.estimated_rewards,
                    date: current_time,
                    action: "claim".to_string(),
                }
            };

            history_store.push(deps.storage, &claim_history)?;
            response_msgs.push(transfer_msg(
                info_sender.to_string(),
                rewards.estimated_rewards,
                None,
                None,
                BLOCK_SIZE,
                reward_contract.code_hash.to_string(),
                reward_contract.address.to_string(),
            )?);
            reward_contract.total_rewards -= rewards.estimated_rewards;
        }
    }

    state.total_staked_amount -= staked.staked_amount;

    let mut transfers: Vec<Transfer> = Vec::new();
    transfers.push(Transfer {
        recipient: info_sender.to_string(),
        token_ids: staked_nfts,
        memo: None,
    });

    let cosmos_batch_msg = batch_transfer_nft_msg(
        transfers,
        None,
        BLOCK_SIZE,
        state.staking_contract.code_hash.clone(),
        state.staking_contract.address.to_string(),
    )?;
    response_msgs.push(cosmos_batch_msg);
    CONFIG_ITEM.save(deps.storage, &state)?;
    STAKED_STORE.insert(
        deps.storage,
        &deps.api.addr_canonicalize(&info_sender.to_string())?,
        &Staked {
            last_claimed_date: None,
            staked_amount: Uint128::from(0u128),
            last_staked_date: None,
            staking_weights: Some(Vec::new()),
        },
    )?;

    STAKED_NFTS_STORE.insert(
        deps.storage,
        &deps.api.addr_canonicalize(&info_sender.to_string())?,
        &Vec::new(),
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

fn try_withdraw_no_reward(
    deps: DepsMut,
    _env: Env,
    info_sender: &Addr,
) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;
    let history_store = HISTORY_STORE.add_suffix(info_sender.to_string().as_bytes());
    let staked = STAKED_STORE
        .get(
            deps.storage,
            &deps.api.addr_canonicalize(&info_sender.to_string())?,
        )
        .ok_or_else(|| StdError::generic_err("You aren't staked"))?;
    let staked_nfts = STAKED_NFTS_STORE
        .get(
            deps.storage,
            &deps.api.addr_canonicalize(&info_sender.to_string())?,
        )
        .ok_or_else(|| StdError::generic_err("NFTs aren't staked"))?;

    if staked.staked_amount == Uint128::from(0u128) {
        return Err(ContractError::CustomError {
            val: "There is nothing to withdraw".to_string(),
        });
    }

    if staked_nfts.len() == 0 {
        return Err(ContractError::CustomError {
            val: "There are no NFTs to withdraw".to_string(),
        });
    }

    let mut response_msgs: Vec<CosmosMsg> = Vec::new();
    let mut transfers: Vec<Transfer> = Vec::new();
    transfers.push(Transfer {
        recipient: info_sender.to_string(),
        token_ids: staked_nfts,
        memo: None,
    });

    let cosmos_batch_msg = batch_transfer_nft_msg(
        transfers,
        None,
        BLOCK_SIZE,
        state.staking_contract.code_hash.clone(),
        state.staking_contract.address.to_string(),
    )?;
    response_msgs.push(cosmos_batch_msg);

    let current_time = _env.block.time.seconds();

    state.total_staked_amount -= staked.staked_amount;
    CONFIG_ITEM.save(deps.storage, &state)?;
    STAKED_STORE.insert(
        deps.storage,
        &deps.api.addr_canonicalize(&info_sender.to_string())?,
        &Staked {
            last_claimed_date: None,
            staked_amount: Uint128::from(0u128),
            last_staked_date: None,
            staking_weights: Some(Vec::new()),
        },
    )?;

    STAKED_NFTS_STORE.insert(
        deps.storage,
        &deps.api.addr_canonicalize(&info_sender.to_string())?,
        &Vec::new(),
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
    let mut state = CONFIG_ITEM.load(deps.storage)?;
    let history_store = HISTORY_STORE.add_suffix(info_sender.to_string().as_bytes());
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
    let current_time = _env.block.time.seconds();
    let rewards_to_claim = get_estimated_rewards(&staked, &current_time, &state)?;
    for rewards in rewards_to_claim.iter() {
        let reward_contract_index = state
            .reward_contracts
            .iter()
            .position(|x| x.name == rewards.reward_contract_name.to_string());
        let reward_contract = &mut state.reward_contracts[reward_contract_index.unwrap()];

        if rewards.estimated_rewards > Uint128::from(0u128) {
            if reward_contract.total_rewards < rewards.estimated_rewards {
                return Err(ContractError::CustomError {
                    val: "Error trying to claim rewards".to_string(),
                });
            }
            let claim_history: History = {
                History {
                    amount: rewards.estimated_rewards,
                    date: current_time,
                    action: "claim".to_string(),
                }
            };

            history_store.push(deps.storage, &claim_history)?;
            response_msgs.push(transfer_msg(
                info_sender.to_string(),
                rewards.estimated_rewards,
                None,
                None,
                BLOCK_SIZE,
                reward_contract.code_hash.to_string(),
                reward_contract.address.to_string(),
            )?);
            staked.last_claimed_date = Some(current_time);
            reward_contract.total_rewards -= rewards.estimated_rewards;
            STAKED_STORE.insert(
                deps.storage,
                &deps.api.addr_canonicalize(&info_sender.to_string())?,
                &staked,
            )?;
            CONFIG_ITEM.save(deps.storage, &state)?;
        } else {
            //this technically should never happen
            return Err(ContractError::CustomError {
                val: "Not allowed to claim yet".to_string(),
            });
        }
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
    contracts: Vec<RewardsContractInfo>,
) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;
    let mut response_msgs: Vec<CosmosMsg> = Vec::new();

    if sender.clone() != state.owner {
        return Err(ContractError::CustomError {
            val: "You don't have the permissions to execute this command".to_string(),
        });
    }

    for reward_contract in state.reward_contracts.iter() {
        if reward_contract.total_rewards != Uint128::from(0u128) {
            return Err(ContractError::CustomError {
                val: "Clear out rewards first before updating".to_string(),
            });
        }
    }

    for reward_contract in state.reward_contracts.iter() {
        response_msgs.push(set_viewing_key_msg(
            state.viewing_key.clone().unwrap().to_string(),
            None,
            BLOCK_SIZE,
            reward_contract.code_hash.to_string(),
            reward_contract.address.to_string(),
        )?);
    }

    state.reward_contracts = contracts;
    CONFIG_ITEM.save(deps.storage, &state)?;

    Ok(Response::new().add_messages(response_msgs))
}

fn try_remove_rewards(deps: DepsMut, sender: &Addr) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;
    let mut response_msgs: Vec<CosmosMsg> = Vec::new();

    if sender.clone() != state.owner {
        return Err(ContractError::CustomError {
            val: "You don't have the permissions to execute this command".to_string(),
        });
    }

    for reward_contract in state.reward_contracts.iter_mut() {
        let cosmos_msg = transfer_msg(
            sender.to_string(),
            reward_contract.total_rewards.clone(),
            None,
            None,
            BLOCK_SIZE,
            reward_contract.code_hash.to_string(),
            reward_contract.address.to_string(),
        )?;
        response_msgs.push(cosmos_msg);

        reward_contract.total_rewards = Uint128::from(0u128);
    }

    CONFIG_ITEM.save(deps.storage, &state)?;
    Ok(Response::new().add_messages(response_msgs))
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

    if sender.clone() != state.owner {
        return Err(ContractError::CustomError {
            val: "You don't have the permissions to execute this command".to_string(),
        });
    }
    state.is_active = is_active;

    CONFIG_ITEM.save(deps.storage, &state)?;

    Ok(Response::default())
}

pub fn try_eject(
    deps: DepsMut,
    _env: Env,
    sender: &Addr,
    staker: &Addr,
) -> Result<Response, ContractError> {
    let mut state = CONFIG_ITEM.load(deps.storage)?;
    if sender.clone() != state.owner {
        return Err(ContractError::CustomError {
            val: "You don't have the permissions to execute this command".to_string(),
        });
    }

    let history_store = HISTORY_STORE.add_suffix(staker.to_string().as_bytes());
    let current_time = _env.block.time.seconds();
    let staked_nfts = STAKED_NFTS_STORE
        .get(
            deps.storage,
            &deps.api.addr_canonicalize(&staker.to_string())?,
        )
        .ok_or_else(|| StdError::generic_err("NFTs aren't staked"))?;

    if staked_nfts.len() == 0 {
        return Err(ContractError::CustomError {
            val: "There are no NFTs to withdraw".to_string(),
        });
    }

    let staked_nfts_len = Uint128::from(staked_nfts.len() as u128);
    let mut response_msgs: Vec<CosmosMsg> = Vec::new();
    let mut transfers: Vec<Transfer> = Vec::new();
    transfers.push(Transfer {
        recipient: staker.to_string(),
        token_ids: staked_nfts,
        memo: None,
    });

    let cosmos_batch_msg = batch_transfer_nft_msg(
        transfers,
        None,
        BLOCK_SIZE,
        state.staking_contract.code_hash.clone(),
        state.staking_contract.address.to_string(),
    )?;
    response_msgs.push(cosmos_batch_msg);

    state.total_staked_amount -= staked_nfts_len;
    CONFIG_ITEM.save(deps.storage, &state)?;
    STAKED_STORE.insert(
        deps.storage,
        &deps.api.addr_canonicalize(&staker.to_string())?,
        &Staked {
            last_claimed_date: None,
            staked_amount: Uint128::from(0u128),
            last_staked_date: None,
            staking_weights: Some(Vec::new()),
        },
    )?;

    STAKED_NFTS_STORE.insert(
        deps.storage,
        &deps.api.addr_canonicalize(&staker.to_string())?,
        &Vec::new(),
    )?;

    let stake_history: History = {
        History {
            amount: staked_nfts_len,
            date: current_time,
            action: "withdraw".to_string(),
        }
    };

    history_store.push(deps.storage, &stake_history)?;
    Ok(Response::new().add_messages(response_msgs))
}

fn get_estimated_rewards(
    staked: &Staked,
    current_time: &u64,
    state: &State,
) -> StdResult<Vec<EstimatedReward>> {
    let mut expected_rewards: Vec<EstimatedReward> = Vec::new();

    for reward_contract in state.reward_contracts.iter() {
        let mut estimated_rewards = Uint128::from(0u128);
        //estimate logic for a normal weighting
        if state.staking_weights.is_none()
            && staked.staked_amount > Uint128::from(0u128)
            && state.total_staked_amount > Uint128::from(0u128)
        {
            let date = if staked.last_claimed_date.is_some() {
                staked.last_claimed_date.unwrap()
            } else {
                staked.last_staked_date.unwrap()
            };

            let user_reward_percentage =
                Decimal::from_ratio(staked.staked_amount, state.total_staked_amount);
            let elapsed_seconds = current_time - date;
            let rewards_per_second =
                Decimal::from_ratio(reward_contract.rewards_per_day, 24u64 * 60u64 * 60u64);
            let est_rewards = user_reward_percentage
                * rewards_per_second
                * Decimal::from_atomics(elapsed_seconds, 0).unwrap();

            let without_decimals = Decimal::from(
                est_rewards * Decimal::from_ratio(1u128, 10u128.pow(est_rewards.decimal_places())),
            );
            estimated_rewards = without_decimals.atomics();
        }
        //estimate logic for when there are weights defined by the nft traits
        else if state.staking_weights.is_some() && staked.staked_amount > Uint128::from(0u128) {
            let date = if staked.last_claimed_date.is_some() {
                staked.last_claimed_date.unwrap()
            } else {
                staked.last_staked_date.unwrap()
            };

            let elapsed_seconds = current_time - date;
            for weight in state.staking_weights.as_ref().unwrap().iter() {
                let user_weight = staked
                    .staking_weights
                    .as_ref()
                    .unwrap()
                    .iter()
                    .find(|&x| x.weight_trait_type == weight.weight_trait_type.to_string());
                let user_reward_percentage =
                    Decimal::from_ratio(user_weight.unwrap().amount, weight.amount);
                let rewards_per_second =
                    Decimal::from_ratio(reward_contract.rewards_per_day, 24u64 * 60u64 * 60u64);
                let percent_of_rps =
                    rewards_per_second * Decimal::percent(weight.weight_percentage.u128() as u64);
                let est_rewards = user_reward_percentage
                    * percent_of_rps
                    * Decimal::from_atomics(elapsed_seconds, 0).unwrap();
                let without_decimals = Decimal::from(
                    est_rewards
                        * Decimal::from_ratio(1u128, 10u128.pow(est_rewards.decimal_places())),
                );
                estimated_rewards = estimated_rewards + without_decimals.atomics();
            }
        }

        let estimated_reward: EstimatedReward = {
            EstimatedReward {
                estimated_rewards: estimated_rewards,
                reward_contract_name: reward_contract.name.to_string(),
            }
        };
        expected_rewards.push(estimated_reward);
    }
    return Ok(expected_rewards);
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
        // total_rewards: state.total_rewards,
        staking_contract: state.staking_contract,
        reward_contracts: Some(state.reward_contracts),
        trait_restriction: state.trait_restriction,
        staking_weights: state.staking_weights,
        is_active: Some(state.is_active),
    })
}

fn query_my_staked(deps: Deps, env: Env, permit: Permit) -> StdResult<MyStakedInfoResponse> {
    let user_raw = get_querier(deps, permit, env.contract.address)?;
    let state = CONFIG_ITEM.load(deps.storage)?;

    let staked = STAKED_STORE.get(deps.storage, &user_raw).unwrap_or(Staked {
        last_claimed_date: None,
        staked_amount: Uint128::from(0u128),
        last_staked_date: None,
        staking_weights: None,
    });

    let current_time = env.block.time.seconds();
    let estimated_rewards = get_estimated_rewards(&staked, &current_time, &state)?;
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

fn query_reward_balance(deps: Deps, env: Env, viewer: ViewerInfo) -> StdResult<Vec<Balance>> {
    check_admin_key(deps, viewer)?;

    let state = CONFIG_ITEM.load(deps.storage)?;
    let mut balances: Vec<Balance> = Vec::new();
    for reward_contract in state.reward_contracts.iter() {
        let balance = balance_query(
            deps.querier,
            env.contract.address.to_string(),
            state.viewing_key.clone().unwrap(),
            BLOCK_SIZE,
            reward_contract.code_hash.to_string(),
            reward_contract.address.to_string(),
        );
        balances.push(balance.unwrap())
    }
    Ok(balances)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::ContractInfo;
    use cosmwasm_std::testing::*;
    use cosmwasm_std::Api;
    use cosmwasm_std::OwnedDeps;
    use std::any::Any;
    #[test]
    fn rewards_calc() {
        //rounding issue makes 1369500000 > 1369499999
        let mut expected = Uint128::from(13694999u128);
        let mut staked: Staked = {
            Staked {
                staked_amount: Uint128::from(1u128),
                last_claimed_date: None,
                last_staked_date: Some(1686588696),
                staking_weights: None,
            }
        };
        let current_time = 1686675096;
        let mut state: State = {
            State {
                owner: Addr::unchecked(""),
                is_active: true,
                staking_contract: {
                    ContractInfo {
                        code_hash: "".to_string(),
                        address: Addr::unchecked(""),
                        name: "".to_string(),
                        stake_type: "".to_string(),
                    }
                },
                reward_contracts: vec![{
                    RewardsContractInfo {
                        code_hash: "".to_string(),
                        address: Addr::unchecked(""),
                        rewards_per_day: Uint128::from(2739000000u128),
                        name: "".to_string(), 
                        total_rewards: Uint128::from(10000000000000u128)
                    }
                }],
                viewing_key: None,
                total_staked_amount: Uint128::from(200u128),
                staking_weights: None,
                trait_restriction: None,
            }
        };
        let x = get_estimated_rewards(&staked, &current_time, &state);
        assert_eq!(x.unwrap()[0].estimated_rewards, expected);

        staked.staked_amount = Uint128::from(100u128);
        expected = Uint128::from(1369499999u128);
        let y = get_estimated_rewards(&staked, &current_time, &state);
        assert_eq!(y.unwrap()[0].estimated_rewards, expected);

        //test weights
        state.staking_weights = Some(vec![
            StakingWeight {
                amount: Uint128::from(33u128),
                weight_trait_type: "Pack".to_string(),
                weight_percentage: Uint128::from(50u128),
            },
            StakingWeight {
                amount: Uint128::from(20086u128),
                weight_trait_type: "Pack Rank".to_string(),
                weight_percentage: Uint128::from(50u128),
            },
        ]);
        state.total_staked_amount = Uint128::from(22322u128);

        staked.staked_amount = Uint128::from(10020u128);
        staked.staking_weights = Some(vec![
            UserStakingWeight {
                amount: Uint128::from(11u128),
                weight_trait_type: "Pack".to_string(),
            },
            UserStakingWeight {
                amount: Uint128::from(10043u128),
                weight_trait_type: "Pack Rank".to_string(),
            },
        ]);
        let z = get_estimated_rewards(&staked, &current_time, &state);
        expected = Uint128::from(1141249998u128);
        assert_eq!(z.unwrap()[0].estimated_rewards, expected);
    }

    // #[test]
    // fn withdraw_test() {
    //     let (init_result, mut deps) = init_helper_with_config();
    //     let state = CONFIG_ITEM.load(&deps.storage).unwrap();
    //     assert_eq!(state.trait_restriction.unwrap(), "Alpha".to_string());

    //     let person = "person".to_string();
    //     let person_raw = deps.api.addr_canonicalize(&person).unwrap();

    //     STAKED_STORE.insert(
    //         &mut deps.storage,
    //         &person_raw,
    //         &Staked {
    //             last_claimed_date: None,
    //             staked_amount: Uint128::from(1 as u128),
    //             last_staked_date: Some(1571711019),
    //             staking_weights: Some(vec![
    //                 UserStakingWeight {
    //                     amount: Uint128::from(11u128),
    //                     weight_trait_type: "Pack".to_string(),
    //                 },
    //                 UserStakingWeight {
    //                     amount: Uint128::from(10043u128),
    //                     weight_trait_type: "Pack Rank".to_string(),
    //                 },
    //             ]),
    //         },
    //     );
    //     STAKED_NFTS_STORE.insert(&mut deps.storage, &person_raw, &vec!["1".to_string()]);
    //     let execute_msg = ExecuteMsg::WithdrawFunds {};
    //     let _handle_result = execute(
    //         deps.as_mut(),
    //         mock_env(),
    //         mock_info("person", &[]),
    //         execute_msg,
    //     );

    //     let error = extract_error_msg(_handle_result);
    //     assert!(error.contains("The"));
    // }
    fn extract_error_msg(error: Result<Response, ContractError>) -> String {
        match error {
            Ok(_response) => panic!("Expected error, but had Ok response"),
            Err(err) => match err {
                ContractError::CustomError { val, .. } => val,
                _ => panic!("Unexpected error result {:?}", err),
            },
        }
    }
    fn init_helper_with_config() -> (
        StdResult<Response>,
        OwnedDeps<MockStorage, MockApi, MockQuerier>,
    ) {
        let mut deps = mock_dependencies();

        let env = mock_env();

        let info = mock_info("instantiator", &[]);
        let init_msg = InstantiateMsg {
            entropy: "sec721".to_string(),
            staking_contract: {
                ContractInfo {
                    code_hash: "".to_string(),
                    address: Addr::unchecked(""),
                    name: "".to_string(),
                    stake_type: "".to_string(),
                }
            },
            reward_contracts: vec![{
                RewardsContractInfo {
                    code_hash: "".to_string(),
                    address: Addr::unchecked(""),
                    rewards_per_day: Uint128::from(2739000000u128),
                    name: "".to_string(),
                    total_rewards: Uint128::from(10000000000000u128),
                }
            }],
            trait_restriction: Some("Alpha".to_string()),
            staking_weights: Some(vec![
                StakingWeight {
                    amount: Uint128::from(33u128),
                    weight_trait_type: "Pack".to_string(),
                    weight_percentage: Uint128::from(50u128),
                },
                StakingWeight {
                    amount: Uint128::from(20086u128),
                    weight_trait_type: "Pack Rank".to_string(),
                    weight_percentage: Uint128::from(50u128),
                },
            ]),
        };

        (instantiate(deps.as_mut(), env, info, init_msg), deps)
    }
}
