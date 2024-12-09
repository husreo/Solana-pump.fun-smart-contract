use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::Instruction, program::{invoke_signed, invoke}, system_instruction};
use anchor_spl::associated_token;
use crate::constants::fee::{VAULT_SEED, METEORA_PROGRAM_KEY};
use std::str::FromStr;
use crate::state::{meteora::{get_pool_create_ix_data, get_function_hash, get_lock_lp_ix_data}, bonding_curve::*, fee_vault::FeeVault};
use crate::{
    errors::ContractError,
    state::global::*,
};

#[derive(Accounts)]
pub struct InitializePoolWithConfig<'info> {
    #[account(
        mut,
        seeds = [Global::SEED_PREFIX.as_bytes()],
        constraint = global.initialized == true @ ContractError::NotInitialized,
        bump,
    )]
    global: Box<Account<'info, Global>>,

    #[account(
        mut,
        seeds = [BondingCurve::SEED_PREFIX.as_bytes(), token_b_mint.to_account_info().key.as_ref()],
        constraint = bonding_curve.complete == false @ ContractError::BondingCurveComplete,
        bump,
    )]
    bonding_curve: Box<Account<'info, BondingCurve>>,

    #[account(
        seeds = [VAULT_SEED], 
        bump
    )]
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub vault: AccountInfo<'info>,
    #[account(mut)]
    /// CHECK: migration vault account where fee is deposited accounts
    pub migration_vault: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Pool account (PDA address)
    pub pool: UncheckedAccount<'info>,

    /// CHECK: Pool account (PDA address)
    pub config: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Config for fee
    pub lp_mint: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Token A LP
    pub a_vault_lp: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Token A LP
    pub b_vault_lp: UncheckedAccount<'info>,
    /// CHECK: Token A mint
    pub token_a_mint: UncheckedAccount<'info>,
    /// CHECK: Token B mint
    pub token_b_mint: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Vault accounts for token A
    pub a_vault: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Vault accounts for token B
    pub b_vault: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Vault LP accounts and mints
    pub a_token_vault: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Vault LP accounts and mints for token B
    pub b_token_vault: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Vault LP accounts and mints for token A
    pub a_vault_lp_mint: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Vault LP accounts and mints for token B
    pub b_vault_lp_mint: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Accounts to bootstrap the pool with initial liquidity
    pub payer_token_a: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Accounts to bootstrap the pool with initial liquidity
    pub payer_token_b: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Accounts to bootstrap the pool with initial liquidity
    pub payer_pool_lp: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Protocol fee token accounts
    pub protocol_token_a_fee: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Protocol fee token accounts
    pub protocol_token_b_fee: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Admin account
    pub payer: Signer<'info>,
    /// CHECK: LP mint metadata PDA. Metaplex do the checking.
    #[account(mut)]
    pub mint_metadata: UncheckedAccount<'info>,
    /// CHECK: Additional program accounts
    pub rent: UncheckedAccount<'info>,
    /// CHECK: Metadata program account
    pub metadata_program: UncheckedAccount<'info>,
    
    /// CHECK: Vault program account
    pub vault_program: UncheckedAccount<'info>,
    /// CHECK: Token program account
    pub token_program: UncheckedAccount<'info>,
    /// CHECK: Associated token program account
    pub associated_token_program: UncheckedAccount<'info>,
    /// CHECK: System program account
    pub system_program: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: 
    pub meteora_program: AccountInfo<'info>,
    /// CHECK: Meteora Event Autority
    pub event_authority: AccountInfo<'info>
}

pub fn initialize_pool_with_config(
    ctx: Context<InitializePoolWithConfig>,
    token_a_amount: u64,
    token_b_amount: u64,
) -> Result<()> {
    let _clientbump = ctx.bumps.vault.to_le_bytes();
    let signer_seeds: &[&[&[u8]]] = &[
        &[VAULT_SEED, _clientbump.as_ref()]
    ];
    let meteora_program_id: Pubkey = Pubkey::from_str(METEORA_PROGRAM_KEY).unwrap();

    msg!("Passed Accounts");

    let mut accounts = vec![
        // meteora accounts
    ];
    accounts.extend(
        ctx.remaining_accounts.iter().map(|acc| AccountMeta {
            pubkey: *acc.key,
            is_signer: false,
            is_writable: true
        })
    );
    let data = get_pool_create_ix_data(
        token_a_amount,
        token_b_amount,
    );

    let instruction = Instruction {
        program_id: meteora_program_id,
        accounts,
        data,
    };

    msg!("Passed Prepare for MT");

    invoke_signed(
        &instruction,
        &[
           // meteora accounts
        ],
        signer_seeds
    )?;
    msg!("Done MT");

    let _ = pay_launch_fee(ctx);
    Ok(())
}

pub fn pay_launch_fee(ctx: Context<InitializePoolWithConfig>) -> Result<()> {
    // transfer SOL to fee recipient
    // sender is signer, must go through system program
    let fee_to = ctx.accounts.migration_vault.clone();
    let fee_from = ctx.accounts.bonding_curve.clone();
    let fee_amount = ctx.accounts.global.migrate_fee_amount;

    ctx.accounts
            .bonding_curve
            .sub_lamports(fee_amount)
            .unwrap();
        ctx.accounts
            .migration_vault
            .add_lamports(fee_amount)
            .unwrap();
    msg!("CreateBondingCurve::pay_launch_fee: done");
    Ok(())
}