use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::Instruction, program::invoke_signed};
use crate::constants::{VAULT_SEED, METEORA_PROGRAM_KEY, QUOTE_MINT};
use std::str::FromStr;
use crate::state::{meteora::get_pool_create_ix_data, bonding_curve::*};
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
    /// CHECK: Vault accounts
    pub vault: AccountInfo<'info>,

    #[account(mut)]
    /// CHECK: Migration vault account where fee is deposited accounts
    pub migration_vault: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Pool account (PDA address)
    pub pool: UncheckedAccount<'info>,

    /// CHECK: Config for fee
    pub config: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: lp mint
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
    /// CHECK: Protocol fee token a accounts
    pub protocol_token_a_fee: UncheckedAccount<'info>,
    
    #[account(mut)]
    /// CHECK: Protocol fee token b accounts
    pub protocol_token_b_fee: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Admin account
    pub payer: Signer<'info>,
    
    #[account(mut)]
    /// CHECK: LP mint metadata PDA. Metaplex do the checking.
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
    /// CHECK: Meteora Program 
    pub meteora_program: AccountInfo<'info>,
    
    /// CHECK: Meteora Event Autority
    pub event_authority: AccountInfo<'info>
}

pub fn initialize_pool_with_config(
    ctx: Context<InitializePoolWithConfig>,
    token_a_amount: u64,
    token_b_amount: u64,
) -> Result<()> {
    let quote_mint: Pubkey = Pubkey::from_str(QUOTE_MINT).unwrap();

    require!(
        ctx.accounts.bonding_curve.mint.key() == ctx.accounts.token_b_mint.key(),
        ContractError::NotBondingCurveMint
    );

    require!(
        quote_mint.key() == ctx.accounts.token_a_mint.key(),
        ContractError::NotSOL
    );

    require!(
        ctx.accounts.global.meteora_config.key() == ctx.accounts.config.key(),
        ContractError::InvalidConfig
    );

    require!(
        !ctx.accounts.bonding_curve.complete,
        ContractError::NotCompleted
    );

    let _clientbump = ctx.bumps.vault.to_le_bytes();
    let signer_seeds: &[&[&[u8]]] = &[
        &[VAULT_SEED, _clientbump.as_ref()]
    ];
    let meteora_program_id: Pubkey = Pubkey::from_str(METEORA_PROGRAM_KEY).unwrap();

    let mut accounts = vec![
        AccountMeta::new(ctx.accounts.pool.key(), false),
        AccountMeta::new_readonly(ctx.accounts.config.key(), false),
        AccountMeta::new(ctx.accounts.lp_mint.key(), false),
        AccountMeta::new_readonly(ctx.accounts.token_a_mint.key(), false),
        AccountMeta::new_readonly(ctx.accounts.token_b_mint.key(), false),
        AccountMeta::new(ctx.accounts.a_vault.key(), false),
        AccountMeta::new(ctx.accounts.b_vault.key(), false),
        AccountMeta::new(ctx.accounts.a_token_vault.key(), false),
        AccountMeta::new(ctx.accounts.b_token_vault.key(), false),
        AccountMeta::new(ctx.accounts.a_vault_lp_mint.key(), false),
        AccountMeta::new(ctx.accounts.b_vault_lp_mint.key(), false),
        AccountMeta::new(ctx.accounts.a_vault_lp.key(), false),
        AccountMeta::new(ctx.accounts.b_vault_lp.key(), false),
        AccountMeta::new(ctx.accounts.payer_token_a.key(), false),
        AccountMeta::new(ctx.accounts.payer_token_b.key(), false),
        AccountMeta::new(ctx.accounts.payer_pool_lp.key(), false),
        AccountMeta::new(ctx.accounts.protocol_token_a_fee.key(), false),
        AccountMeta::new(ctx.accounts.protocol_token_b_fee.key(), false),
        AccountMeta::new(ctx.accounts.payer.key(), true),
        AccountMeta::new_readonly(ctx.accounts.rent.key(), false),
        AccountMeta::new(ctx.accounts.mint_metadata.key(), false),
        AccountMeta::new_readonly(ctx.accounts.metadata_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.vault_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.token_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.associated_token_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
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

    invoke_signed(
        &instruction,
        &[
            ctx.accounts.pool.to_account_info(),
            ctx.accounts.config.to_account_info(),
            ctx.accounts.lp_mint.to_account_info(),
            ctx.accounts.token_a_mint.to_account_info(),
            ctx.accounts.token_b_mint.to_account_info(),
            ctx.accounts.a_vault.to_account_info(),
            ctx.accounts.b_vault.to_account_info(),
            ctx.accounts.a_token_vault.to_account_info(),
            ctx.accounts.b_token_vault.to_account_info(),
            ctx.accounts.a_vault_lp_mint.to_account_info(),
            ctx.accounts.b_vault_lp_mint.to_account_info(),
            ctx.accounts.a_vault_lp.to_account_info(),
            ctx.accounts.b_vault_lp.to_account_info(),
            ctx.accounts.payer_token_a.to_account_info(),
            ctx.accounts.payer_token_b.to_account_info(),
            ctx.accounts.payer_pool_lp.to_account_info(),
            ctx.accounts.protocol_token_a_fee.to_account_info(),
            ctx.accounts.protocol_token_b_fee.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.rent.to_account_info(),
            ctx.accounts.mint_metadata.to_account_info(),
            ctx.accounts.metadata_program.to_account_info(),
            ctx.accounts.vault_program.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.associated_token_program.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        signer_seeds
    )?;

    let _ = pay_launch_fee(ctx);
    Ok(())
}

pub fn pay_launch_fee(ctx: Context<InitializePoolWithConfig>) -> Result<()> {
    // transfer SOL to fee recipient
    // sender is signer, must go through system program
    let fee_amount = ctx.accounts.global.migrate_fee_amount;

    ctx.accounts
            .bonding_curve
            .sub_lamports(fee_amount)
            .unwrap();
        ctx.accounts
            .migration_vault
            .add_lamports(fee_amount)
            .unwrap();
    Ok(())
}