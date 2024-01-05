use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount, transfer, Transfer},
};
declare_id!("Hob4uvwXPsgvCykQY2DBS3TV4WDtdAy4m9UVGT2DA9Y3");

#[program]
pub mod seafarers_spl_staking {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, bump: u8) -> Result<()> {
        msg!("Instruction: Initialize");

        msg!("before staking pool");
        let staking_pool: &mut Account<'_, StakingPool> = &mut ctx.accounts.staking_pool;
        msg!("after staking pool");
        staking_pool.owner = ctx.accounts.owner.key();
        msg!("Staking Pool Owner: {:?}", staking_pool.owner);
        staking_pool.staking_token_mint = ctx.accounts.stake_token_mint.key();
        msg!("Staking Pool Mint: {:?}", staking_pool.staking_token_mint);
        staking_pool.total_staked_amount = 0;
        msg!("Staking Pool Total Staked Amount: {:?}", staking_pool.total_staked_amount);
        staking_pool.total_reward_amount = 0;
        msg!("Staking Pool Total Reward Amount: {:?}", staking_pool.total_reward_amount);
        staking_pool.total_user_count = 0;
        msg!("Staking Pool Total User Count: {:?}", staking_pool.total_user_count);
        staking_pool.reward_per_second = 0;
        msg!("Staking Pool Reward Per Second: {:?}", staking_pool.reward_per_second);
        staking_pool.bump = bump;
        msg!("Staking Pool Bump: {:?}", staking_pool.bump);

        Ok(())
    }

    pub fn fund(ctx: Context<Fund>, amount: u64) -> Result<()> {
        msg!("Instruction: Fund");

        let staking_pool: &mut Account<'_, StakingPool> = &mut ctx.accounts.staking_pool;

        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.funder_ata.to_account_info(),
                    to: ctx.accounts.vault_ata.to_account_info(),
                    authority: ctx.accounts.funder.to_account_info(),
                },
            ),
            amount,
        )?;

        staking_pool.total_reward_amount += amount;
        msg!("Staking Pool Total Reward Amount: {:?}", staking_pool.total_reward_amount);

        Ok(())
    }
    
    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        msg!("Instruction: Stake");

        let staking_pool: &mut Account<'_, StakingPool> = &mut ctx.accounts.staking_pool;
        let staker_info: &mut Account<'_, StakerInfo> = &mut ctx.accounts.staker_info;

        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.staker_ata.to_account_info(),
                    to: ctx.accounts.vault_ata.to_account_info(),
                    authority: ctx.accounts.staker.to_account_info(),
                },
            ),
            amount,
        )?;

        staking_pool.total_staked_amount += amount;
        //TODO: only update user count if it is a new unique user wallet
        staking_pool.total_user_count += 1;

        staker_info.amount += amount;
        staker_info.last_stake_timestamp = Clock::get().unwrap().unix_timestamp;

        msg!("Staking Pool Total Staked Amount: {:?}", staking_pool.total_staked_amount);
        msg!("Staking Pool Total User Count: {:?}", staking_pool.total_user_count);
        msg!("Staker Info Amount: {:?}", staker_info.amount);
        msg!("Staker Info Last Stake Timestamp: {:?}", staker_info.last_stake_timestamp);

        Ok(())
    }

    pub fn unstake (ctx: Context<Unstake>, amount: u64) -> Result<()> {
        msg!("Instruction: Unstake");

        let staking_pool: &mut Account<'_, StakingPool> = &mut ctx.accounts.staking_pool;
        let staker_info: &mut Account<'_, StakerInfo> = &mut ctx.accounts.staker_info;

        let rewards_earned = Clock::get().unwrap().unix_timestamp - staker_info.last_stake_timestamp;
        msg!("Rewards Earned: {:?}", rewards_earned);
        let total_amount = if rewards_earned > 0 {
            amount + rewards_earned as u64
        } else {
            amount // or handle negative rewards differently
        };

        msg!("Total Amount: {:?}", total_amount);

        let bump = staking_pool.bump;
        let staking_pool_owner_key = staking_pool.owner.key();
        let seeds = &[b"vault", staking_pool_owner_key.as_ref(), &[bump]];
        let signer = &[&seeds[..]];

        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault_ata.to_account_info(),
                    to: ctx.accounts.staker_ata.to_account_info(),
                    authority: ctx.accounts.token_vault.to_account_info(),
                },
                signer,
            ),
            total_amount,
        )?;

        staking_pool.total_staked_amount -= amount;
        staking_pool.total_reward_amount -= rewards_earned as u64;
        if staker_info.amount == amount {
            staking_pool.total_user_count -= 1;
        }

        staker_info.amount -= amount;
        staker_info.last_stake_timestamp = 0;

        msg!("Staking Pool Total Staked Amount: {:?}", staking_pool.total_staked_amount);
        msg!("Staking Pool Total Reward Amount: {:?}", staking_pool.total_reward_amount);
        msg!("Staking Pool Total User Count: {:?}", staking_pool.total_user_count);
        msg!("Staker Info Amount: {:?}", staker_info.amount);
        msg!("Staker Info Last Stake Timestamp: {:?}", staker_info.last_stake_timestamp);

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init_if_needed,
        seeds=[b"pool", owner.key().as_ref()], 
        bump,
        payer = owner,
        space = StakingPool::LEN + 8)]
    pub staking_pool: Account<'info, StakingPool>,

    pub stake_token_mint: Box<Account<'info, Mint>>,

    #[account(
        seeds=[b"vault", owner.key().as_ref()],
        bump,
    )]
    pub token_vault: SystemAccount<'info>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::authority = token_vault,
        associated_token::mint = stake_token_mint,
    )]
    pub vault_ata: Box<Account<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Fund<'info> {
    #[account(mut)]
    pub funder: Signer<'info>,
    #[account(mut)]
    pub staking_pool: Account<'info, StakingPool>,

    pub stake_token_mint: Box<Account<'info, Mint>>,

    #[account(
        seeds = [b"vault", staking_pool.owner.key().as_ref()],
        bump = staking_pool.bump,
    )]
    pub token_vault: SystemAccount<'info>,

    #[account(
        init_if_needed,
        payer = funder,
        associated_token::authority = funder,
        associated_token::mint = stake_token_mint,
    )]
    pub funder_ata: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = funder,
        associated_token::authority = token_vault,
        associated_token::mint = stake_token_mint,
    )]
    pub vault_ata: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
   #[account(mut)]
    pub staker: Signer<'info>,
    
    #[account(mut)]
    pub staking_pool: Account<'info, StakingPool>,

    #[account(
        init_if_needed, 
        seeds=[b"stake", staker.key().as_ref()], 
        bump,
        payer = staker, 
        space = 8 + StakerInfo::LEN)]
    pub staker_info: Box<Account<'info, StakerInfo>>,

    #[account(
        seeds = [b"vault", staking_pool.owner.key().as_ref()],
        bump = staking_pool.bump,
    )]
    pub token_vault: SystemAccount<'info>,

    #[account(
        mut,
        associated_token::authority = staker,
        associated_token::mint = staking_pool.staking_token_mint,
    )]
    pub staker_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::authority = token_vault,
        associated_token::mint = staking_pool.staking_token_mint,
    )]
    pub vault_ata: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(mut)]
    pub staking_pool: Account<'info, StakingPool>,

    #[account(mut)]
    pub staker_info: Account<'info, StakerInfo>,

    #[account(
        seeds = [b"vault", staking_pool.owner.key().as_ref()],
        bump = staking_pool.bump,
    )]
    pub token_vault: SystemAccount<'info>,

    #[account(
        mut,
        associated_token::authority = staker,
        associated_token::mint = staking_pool.staking_token_mint,
    )]
    pub staker_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::authority = token_vault,
        associated_token::mint = staking_pool.staking_token_mint,
    )]
    pub vault_ata: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[account]
pub struct StakingPool {
    pub owner: Pubkey,
    pub staking_token_mint: Pubkey,
    pub total_staked_amount: u64,
    pub total_reward_amount: u64,
    pub total_user_count: u64,
    pub reward_per_second: u64,
    pub bump: u8
}

#[account]
pub struct StakerInfo {
    pub amount: u64,
    pub last_stake_timestamp: i64,
}


impl StakerInfo {
    pub const LEN: usize = 8 + 8;
}

impl StakingPool {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + 8 + 1;
}