use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{UnorderedMap, UnorderedSet};
use near_sdk::json_types::U128;
use near_sdk::{assert_one_yocto, env, near_bindgen, AccountId, Balance, BorshStorageKey, Promise};

pub type SnipeId = u64;
pub type TokenId = String;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Snipe {
    snipe_id: SnipeId,
    account_id: AccountId,
    contract_id: AccountId,
    token_id: Option<TokenId>,
    price: Balance,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
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
        self.snipe_by_id.get(&snipe_id).expect("errors.snipe not found")
    }

    // payable

    #[payable]
    pub fn snipe_token(&mut self, contract_id: AccountId, token_id: Option<TokenId>) {
        assert_one_yocto();

        let account_id = env::predecessor_account_id();
        let attached_deposit = env::attached_deposit();

        let id = self.snipes_by_account_id.len() + 1;
        self.snipe_by_id
            .insert(
                &id,
                &Snipe {
                    snipe_id: id,
                    account_id: account_id.clone(),
                    contract_id,
                    token_id,
                    price: attached_deposit,
                },
            )
            .expect("errors.failed to insert snipe");

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
    }

    #[payable]
    pub fn delete_snipe(&mut self, snipe_id: SnipeId) {
        assert_one_yocto();

        let account_id = env::predecessor_account_id();
        let snipe = self
            .snipe_by_id
            .get(&snipe_id)
            .expect("errors.snipe not found");
        assert!(
            snipe.account_id == account_id,
            "errors.only owner can delete snipe"
        );

        self.snipe_by_id.remove(&snipe_id);
        let mut snipes_per_account_id = self
            .snipes_by_account_id
            .get(&account_id)
            .expect("errors.snipes snipes_per_account_id not found");
        snipes_per_account_id.remove(&snipe_id);
        self.snipes_by_account_id
            .insert(&account_id, &snipes_per_account_id);

        self.internal_transfer_near(account_id, snipe.price);
    }

    // private functions

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
}
