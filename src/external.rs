use crate::{AccountId, TokenId, SnipeId};
use near_sdk::{ext_contract, json_types::U128, Balance};

#[ext_contract(this_contract)]
trait Callbacks {
  fn resolve_buy(snipe_id: SnipeId, price: Balance);
}

#[ext_contract(paras_marketplace)]
trait ParasMarketplace {
    fn buy(
        nft_contract_id: AccountId,
        token_id: TokenId,
        ft_token_id: Option<AccountId>,
        price: Option<U128>,
    );
}
