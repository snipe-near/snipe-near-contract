use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{near_bindgen, AccountId};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
}

#[near_bindgen]
impl Contract {

  #[init]
  #[private]
  pub fn init(ownerId: AccountId) -> Self {
    Self {
    }
  }
}
