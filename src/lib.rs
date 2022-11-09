use events::{LogBuyToken, LogDeleteSnipe, LogSnipe, NearEvent};
use external::{nft_contract, paras_marketplace, mintbase_marketplace};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{UnorderedMap, UnorderedSet};
use near_sdk::env::panic_str;
use near_sdk::json_types::U128;
use near_sdk::serde::Serialize;
use near_sdk::{
    assert_one_yocto, env, is_promise_success, near_bindgen, require, AccountId, Balance,
    BorshStorageKey, Gas, PanicOnDefault, Promise,
};
use serde::Deserialize;

const GAS_FOR_BUY_TOKEN: Gas = Gas(200_000_000_000_000);
const GAS_FOR_RESOLVE_BUY: Gas = Gas(30_000_000_000_000);
const GAS_FOR_NFT_TRANSFER: Gas = Gas(30_000_000_000_000);

pub mod events;
pub mod external;

pub type SnipeId = u64;
pub type TokenId = String;

pub enum NftMarketplace {
    Paras,
    Mintbase,
    Fewfar,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug)]
pub enum SnipeStatus {
    Waiting,
    Sniping,
    Success,
    Failed,
    Deleted,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize)]
pub struct Snipe {
    snipe_id: SnipeId,
    account_id: AccountId,
    contract_id: AccountId,
    token_id: Option<TokenId>,
    deposit: Balance,
    status: SnipeStatus,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    owner_id: AccountId,
    snipe_by_id: UnorderedMap<SnipeId, Snipe>,
    snipes_by_account_id: UnorderedMap<AccountId, UnorderedSet<SnipeId>>,
}

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    SnipeById,
    SnipeByAccountId,
    SnipesPerAccountId { account_id: AccountId },
}

#[near_bindgen]
impl Contract {
    #[init]
    #[private]
    pub fn init(owner_id: AccountId) -> Self {
        Self {
            owner_id,
            snipe_by_id: UnorderedMap::new(StorageKey::SnipeById),
            snipes_by_account_id: UnorderedMap::new(StorageKey::SnipeByAccountId),
        }
    }

    // views

    pub fn snipes_by_account_id(
        &self,
        account_id: AccountId,
        from_index: Option<U128>,
        limit: Option<u64>,
    ) -> Vec<Snipe> {
        let snipes = self
            .snipes_by_account_id
            .get(&account_id)
            .expect("errors.account_id not found");

        let start_index: u128 = from_index.map(From::from).unwrap_or_default();
        let limit = limit.map(|v| v as usize).unwrap_or(usize::MAX);

        snipes
            .iter()
            .skip(start_index as usize)
            .take(limit)
            .map(|snipe_id| {
                self.snipe_by_id
                    .get(&snipe_id)
                    .expect("errors.snipe_id not found")
            })
            .collect()
    }

    pub fn snipe_by_id(&self, snipe_id: SnipeId) -> Snipe {
        self.snipe_by_id
            .get(&snipe_id)
            .expect("errors.snipe not found")
    }

    // payable

    #[payable]
    pub fn snipe(
        &mut self,
        contract_id: AccountId,
        token_id: Option<TokenId>,
        memo: Option<String>,
    ) {
        self.assert_more_than_one_yocto();

        let account_id = env::predecessor_account_id();
        let attached_deposit = env::attached_deposit();

        let id = self.snipe_by_id.len() + 1;
        let snipe = Snipe {
            snipe_id: id.clone(),
            account_id: account_id.clone(),
            contract_id: contract_id.clone(),
            token_id: token_id.clone(),
            deposit: attached_deposit,
            status: SnipeStatus::Waiting,
        };
        self.snipe_by_id.insert(&id, &snipe);

        let mut snipes_per_account_id =
            self.snipes_by_account_id
                .get(&account_id)
                .unwrap_or_else(|| {
                    UnorderedSet::new(StorageKey::SnipesPerAccountId {
                        account_id: account_id.clone(),
                    })
                });

        snipes_per_account_id.insert(&id);
        self.snipes_by_account_id
            .insert(&account_id, &snipes_per_account_id);

        NearEvent::log_snipe(LogSnipe {
            snipe_id: id,
            account_id: account_id.to_string(),
            contract_id: contract_id.to_string(),
            token_id,
            deposit: attached_deposit.to_string(),
            status: SnipeStatus::Waiting,
            memo,
        })
    }

    #[payable]
    pub fn delete_snipe(&mut self, snipe_id: SnipeId) {
        assert_one_yocto();

        let account_id = env::predecessor_account_id();
        let mut snipe = self
            .snipe_by_id
            .get(&snipe_id)
            .expect("errors.snipe not found");
        assert_eq!(
            snipe.account_id, account_id,
            "errors.only owner can delete snipe"
        );

        if !matches!(snipe.status, SnipeStatus::Waiting) {
            panic_str("errors.snipe is not in waiting status");
        }

        snipe.status = SnipeStatus::Deleted;
        self.snipe_by_id.insert(&snipe_id, &snipe);

        if snipe.deposit > 0 {
            self.internal_transfer_near(account_id.clone(), snipe.deposit);
        }

        NearEvent::log_delete_snipe(LogDeleteSnipe {
            snipe_id,
            account_id: account_id.to_string(),
        });
    }

    #[payable]
    pub fn buy_token(
        &mut self,
        marketplace_contract_id: AccountId,
        price: U128,
        snipe_id: SnipeId,
        token_id: Option<TokenId>,
    ) -> Promise {
        self.assert_owner();

        let mut snipe = self
            .snipe_by_id
            .get(&snipe_id)
            .expect("errors.snipe not found");
        if !matches!(snipe.status, SnipeStatus::Waiting) {
            panic_str("errors.snipe is not in waiting status");
        }
        snipe.status = SnipeStatus::Sniping;
        self.snipe_by_id.insert(&snipe_id, &snipe);

        let target_token_id: TokenId;
        if snipe.token_id.is_some() {
            target_token_id = snipe.token_id.as_ref().unwrap().clone();
        } else {
            target_token_id = token_id.expect("errors.token_id is required");
        }

        if price.0 > snipe.deposit {
            panic_str("errors.price is more than snipe deposit")
        }

        let nft_marketplace = self
            .get_nft_marketplace_by_contract(marketplace_contract_id.clone())
            .expect("errros.marketplace not found");
        match nft_marketplace {
            NftMarketplace::Paras => self.internal_buy_from_paras(
                marketplace_contract_id,
                price,
                &snipe,
                target_token_id,
            ),
            NftMarketplace::Mintbase => self.internal_buy_from_mintbase(
                marketplace_contract_id,
                price,
                &snipe,
                target_token_id,
            ),
            _ => {
                panic_str("errors.marketplace not supported");
            }
        }
    }

    #[private]
    pub fn resolve_buy(
        &mut self,
        marketplace_contract_id: AccountId,
        snipe_id: SnipeId,
        price: U128,
        token_id: TokenId,
    ) -> Promise {
        let mut snipe = self
            .snipe_by_id
            .get(&snipe_id)
            .expect("errors.snipe not found");

        if !is_promise_success() {
            snipe.status = SnipeStatus::Failed;
            self.snipe_by_id.insert(&snipe_id, &snipe);

            NearEvent::log_buy_token(LogBuyToken {
                marketplace_contract_id: marketplace_contract_id.to_string(),
                price: price.0.to_string(),
                snipe_id,
                token_id,
                status: SnipeStatus::Failed,
                account_id: snipe.account_id.to_string(),
            });

            panic_str("errors.buy token failed")
        }

        let refund_deposit = snipe.deposit - price.0;
        if refund_deposit > 0 {
            self.internal_transfer_near(snipe.account_id.clone(), refund_deposit);
        }

        snipe.status = SnipeStatus::Success;
        self.snipe_by_id.insert(&snipe_id, &snipe);

        NearEvent::log_buy_token(LogBuyToken {
            marketplace_contract_id: marketplace_contract_id.to_string(),
            price: price.0.to_string(),
            snipe_id,
            token_id: token_id.clone(),
            status: SnipeStatus::Success,
            account_id: snipe.account_id.to_string(),
        });

        nft_contract::ext(snipe.contract_id)
            .with_attached_deposit(1)
            .with_static_gas(GAS_FOR_NFT_TRANSFER)
            .nft_transfer(snipe.account_id, token_id, None, None)
    }

    // private functions

    fn internal_buy_from_paras(
        &mut self,
        marketplace_contract_id: AccountId,
        price: U128,
        snipe: &Snipe,
        token_id: TokenId,
    ) -> Promise {
        let nft_contract_id = snipe.contract_id.clone();

        paras_marketplace::ext(marketplace_contract_id.clone())
            .with_static_gas(GAS_FOR_BUY_TOKEN)
            .with_attached_deposit(price.0)
            .buy(
                nft_contract_id,
                token_id.clone(),
                None,
                Some(U128(price.0.clone())),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_RESOLVE_BUY)
                    .resolve_buy(marketplace_contract_id, snipe.snipe_id, price, token_id),
            )
    }

    fn internal_buy_from_mintbase(
        &mut self,
        marketplace_contract_id: AccountId,
        price: U128,
        snipe: &Snipe,
        token_id: TokenId,
    ) -> Promise {
        let nft_contract_id = snipe.contract_id.clone();

        mintbase_marketplace::ext(marketplace_contract_id.clone())
            .with_static_gas(GAS_FOR_BUY_TOKEN)
            .with_attached_deposit(price.0)
            .buy(
                nft_contract_id,
                token_id.clone(),
            )
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_RESOLVE_BUY)
                    .resolve_buy(marketplace_contract_id, snipe.snipe_id, price, token_id),
            )
    }

    fn internal_transfer_near(&self, account_id: AccountId, amount: Balance) {
        let balance = env::account_balance();
        if balance < amount {
            env::panic_str(&format!(
                "errors.not enough balance to transfer near, balance: {}, amount: {}",
                balance, amount
            ));
        }
        Promise::new(account_id).transfer(amount);
    }

    fn assert_more_than_one_yocto(&self) {
        require!(
            env::attached_deposit() > 1,
            "errors.attached deposit should be more than 1 yoctoNEAR"
        )
    }

    fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "errors.owner only"
        )
    }

    fn get_nft_marketplace_by_contract(
        &self,
        marketplace_contract_id: AccountId,
    ) -> Option<NftMarketplace> {
        match marketplace_contract_id.as_str() {
            "paras-marketplace-v1.testnet" => Some(NftMarketplace::Paras),
            "marketplace.paras.mainnet" => Some(NftMarketplace::Paras),
            "market-v2-beta.mintspace2.testnet" => Some(NftMarketplace::Mintbase),
            "market.mintbase1.near" => Some(NftMarketplace::Mintbase),
            _ => None,
        }
    }
}
