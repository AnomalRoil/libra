//! account: bob, 10000Coin1

//! new-transaction
//! sender: bob
//! max-gas: 1000
//! gas-price: 1
//! gas-currency: Coin1
script {
    fun main() { while (true) {} }
}
// check: OUT_OF_GAS

//! new-transaction
//! sender: blessed
//! type-args: 0x1::Coin1::Coin1
script {
use 0x1::TransactionFee;
use 0x1::Roles::{Self, TreasuryComplianceRole};
fun burn_txn_fees<CoinType>(blessed_account: &signer) {
    let tc_capability =
        Roles::extract_privilege_to_capability<TreasuryComplianceRole>(blessed_account);
    TransactionFee::burn_fees<CoinType>(blessed_account, &tc_capability);
    Roles::restore_capability_to_privilege(blessed_account, tc_capability);
}
}
// check: PreburnEvent
// check: BurnEvent
// check: EXECUTED

//! new-transaction
//! sender: blessed
//! type-args: 0x1::Coin1::Coin1
script {
use 0x1::TransactionFee;
use 0x1::Roles::{Self, TreasuryComplianceRole};
fun burn_txn_fees<CoinType>(blessed_account: &signer) {
    let tc_capability =
        Roles::extract_privilege_to_capability<TreasuryComplianceRole>(blessed_account);
    TransactionFee::burn_fees<CoinType>(blessed_account, &tc_capability);
    Roles::restore_capability_to_privilege(blessed_account, tc_capability);
}
}
// check: PreburnEvent
// check: BurnEvent
// check: EXECUTED