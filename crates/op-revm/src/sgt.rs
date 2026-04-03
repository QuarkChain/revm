//! Soul Gas Token (SGT) support for OP Stack
//!
//! This module provides SGT balance reading functionality for gas payment.

use revm::primitives::{Address, B256, U256, keccak256};
use revm::context::journaled_state::account::JournaledAccountTr;
use revm::context_interface::JournalTr;
use revm::database_interface::Database;

/// SGT contract predeploy address
pub const SGT_CONTRACT: Address = Address::new([
    0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x08, 0x00,
]);

/// Balance mapping base slot (must match Solidity contract)
pub const SGT_BALANCE_SLOT: u64 = 51;

/// Calculate storage slot for account's SGT balance
///
/// Formula: `keccak256(abi.encode(account, 51))`
pub fn sgt_balance_slot(account: Address) -> B256 {
    let mut data = [0u8; 64];
    // Address (20 bytes) left-padded to 32 bytes
    data[12..32].copy_from_slice(account.as_slice());
    // Slot 51 as U256 (32 bytes big-endian)
    data[32..64].copy_from_slice(&U256::from(SGT_BALANCE_SLOT).to_be_bytes::<32>());
    keccak256(data)
}

/// Read SGT balance from contract storage.
///
/// Uses `_no_warm` journal methods to avoid affecting EIP-2929 warm/cold status,
/// matching op-geth's `GetSoulBalance` which uses `GetState` (no access list warming).
pub fn read_sgt_balance<JOURNAL>(journal: &mut JOURNAL, account: Address) -> Result<U256, <JOURNAL::Database as Database>::Error>
where
    JOURNAL: JournalTr,
{
    journal.load_account_no_warm(SGT_CONTRACT)?;
    let sgt_slot = sgt_balance_slot(account);
    journal.sload_no_warm(SGT_CONTRACT, sgt_slot.into())
}

/// Deduct amount from SGT balance in contract storage.
///
/// This performs: `balance[account] -= amount` in SGT contract storage.
/// When `is_native_backed` is true, also deducts from SGT contract's native balance
/// (matching op-geth's `subSoulBalance` behavior).
///
/// Uses `_no_warm` journal methods to avoid affecting EIP-2929 warm/cold status.
pub fn deduct_sgt_balance<JOURNAL>(journal: &mut JOURNAL, account: Address, amount: U256, is_native_backed: bool) -> Result<(), <JOURNAL::Database as Database>::Error>
where
    JOURNAL: JournalTr,
{
    if amount.is_zero() {
        return Ok(());
    }

    journal.load_account_no_warm(SGT_CONTRACT)?;
    let sgt_slot = sgt_balance_slot(account);
    let sgt_balance = journal.sload_no_warm(SGT_CONTRACT, sgt_slot.into())?;
    let new_sgt = sgt_balance.saturating_sub(amount);
    journal.sstore_no_warm(SGT_CONTRACT, sgt_slot.into(), new_sgt)?;

    if is_native_backed {
        journal.load_account_mut_no_warm(SGT_CONTRACT)?.decr_balance(amount);
    }

    Ok(())
}

/// Collect native balance from a fee amount, burning the SGT portion.
///
/// When SGT is enabled and not native-backed, fees are split between SGT and native pools.
/// The SGT portion is burned (not paid to recipient), while the native portion goes to the
/// recipient. This matches op-geth's `collectNativeBalance`.
///
/// Deducts from `sgt_remaining` first (burned), then from `native_remaining` (to recipient).
/// Both pools are mutated in place. Returns the native amount that should be paid to the recipient.
///
/// Returns `amount` unchanged when `sgt_remaining == 0` or `is_native_backed`.
pub fn collect_native_balance(
    amount: U256,
    is_native_backed: bool,
    sgt_remaining: &mut U256,
    native_remaining: &mut U256,
) -> U256 {
    if is_native_backed || sgt_remaining.is_zero() {
        return amount;
    }

    // Burn from SGT pool first
    let sgt_burn = amount.min(*sgt_remaining);
    *sgt_remaining = sgt_remaining.saturating_sub(sgt_burn);

    // Remainder comes from native pool
    let native_part = amount.saturating_sub(sgt_burn).min(*native_remaining);
    *native_remaining = native_remaining.saturating_sub(native_part);

    native_part
}

/// Add amount to SGT balance in contract storage.
///
/// This performs: `balance[account] += amount` in SGT contract storage.
/// When `is_native_backed` is true, also adds to SGT contract's native balance
/// (matching op-geth's `addSoulBalance` behavior).
///
/// Uses `_no_warm` journal methods to avoid affecting EIP-2929 warm/cold status.
pub fn add_sgt_balance<JOURNAL>(journal: &mut JOURNAL, account: Address, amount: U256, is_native_backed: bool) -> Result<(), <JOURNAL::Database as Database>::Error>
where
    JOURNAL: JournalTr,
{
    if amount.is_zero() {
        return Ok(());
    }

    journal.load_account_no_warm(SGT_CONTRACT)?;
    let sgt_slot = sgt_balance_slot(account);
    let current_sgt = journal.sload_no_warm(SGT_CONTRACT, sgt_slot.into())?;
    let new_sgt = current_sgt.saturating_add(amount);
    journal.sstore_no_warm(SGT_CONTRACT, sgt_slot.into(), new_sgt)?;

    if is_native_backed {
        journal.load_account_mut_no_warm(SGT_CONTRACT)?.incr_balance(amount);
    }

    Ok(())
}
