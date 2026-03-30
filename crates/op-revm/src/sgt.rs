//! Soul Gas Token (SGT) support for OP Stack
//!
//! This module provides SGT balance reading functionality for gas payment.

use revm::primitives::{Address, B256, U256, keccak256};
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

/// Read SGT balance from contract storage
pub fn read_sgt_balance<JOURNAL>(journal: &mut JOURNAL, account: Address) -> Result<U256, <JOURNAL::Database as Database>::Error>
where
    JOURNAL: JournalTr,
{
    journal.load_account(SGT_CONTRACT)?;
    let sgt_slot = sgt_balance_slot(account);
    let state_load = journal.sload(SGT_CONTRACT, sgt_slot.into())?;
    Ok(state_load.data)
}

/// Deduct amount from SGT balance in contract storage
///
/// This performs: `balance[account] -= amount` in SGT contract storage.
pub fn deduct_sgt_balance<JOURNAL>(journal: &mut JOURNAL, account: Address, amount: U256) -> Result<(), <JOURNAL::Database as Database>::Error>
where
    JOURNAL: JournalTr,
{
    if amount.is_zero() {
        return Ok(());
    }

    journal.load_account(SGT_CONTRACT)?;
    let sgt_slot = sgt_balance_slot(account);
    let state_load = journal.sload(SGT_CONTRACT, sgt_slot.into())?;
    let sgt_balance = state_load.data;
    let new_sgt = sgt_balance.saturating_sub(amount);
    journal.sstore(SGT_CONTRACT, sgt_slot.into(), new_sgt)?;

    Ok(())
}

/// Add amount to SGT balance in contract storage
///
/// This performs: `balance[account] += amount` in SGT contract storage.
pub fn add_sgt_balance<JOURNAL>(journal: &mut JOURNAL, account: Address, amount: U256) -> Result<(), <JOURNAL::Database as Database>::Error>
where
    JOURNAL: JournalTr,
{
    if amount.is_zero() {
        return Ok(());
    }

    journal.load_account(SGT_CONTRACT)?;
    let sgt_slot = sgt_balance_slot(account);
    let state_load = journal.sload(SGT_CONTRACT, sgt_slot.into())?;
    let current_sgt = state_load.data;
    let new_sgt = current_sgt.saturating_add(amount);
    journal.sstore(SGT_CONTRACT, sgt_slot.into(), new_sgt)?;

    Ok(())
}
