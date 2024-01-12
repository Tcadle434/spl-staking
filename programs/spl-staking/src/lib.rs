use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount, transfer, Transfer},
};
declare_id!("3RfqbVcgDYvZ9JhVBZyfMpoocqiaLBVEAdb6wyANhRdx");

#[program]
pub mod spl_staking {
    use super::*;

    /* 
    Initialize the staking pool. This is a PDA that will be used to store key information about the token 
    being staked, the total amount of tokens staked, the total amount of rewards, and the reward rate, etc.
    */
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Instruction: Initialize");

        let staking_pool: &mut Account<'_, StakingPool> = &mut ctx.accounts.staking_pool;
        staking_pool.owner = ctx.accounts.owner.key();
        staking_pool.staking_token_mint = ctx.accounts.stake_token_mint.key();
        staking_pool.total_staked_amount = 0;
        staking_pool.total_reward_amount = 0;
        staking_pool.reward_per_second = 0;
        staking_pool.bump =  ctx.bumps.staking_pool;

        Ok(())
    }

    /*
    Fund the staking pool. This will transfer the amount of tokens specified by the user to the staking pool.
    total_reward_amount tracks the amount of funds added to the staking pool that are available to be distributed
    as rewards.
     */
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

        staking_pool.total_reward_amount = staking_pool.total_reward_amount.checked_add(amount).unwrap();
        msg!("Staking Pool Total Reward Amount: {:?}", staking_pool.total_reward_amount);

        Ok(())
    }
    
    /*
    Stake a user's tokens. This will transfer the amount of tokens specified by the user
    from their wallet to the ATA of the token_vault. All user information and pool information
    are updated accordingly.
     */
    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        msg!("Instruction: Stake");

        require_gt!(amount, 0, ErrorCode::ZeroStake);

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

        staking_pool.total_staked_amount = staking_pool.total_staked_amount.checked_add(amount).unwrap();
        staker_info.staked_amount = staker_info.staked_amount.checked_add(amount).unwrap();

        let now: u64 = Clock::get().unwrap().unix_timestamp.try_into().unwrap();

        // if there is not currently an active stake
        if staker_info.last_stake_timestamp == 0 {
            staker_info.earned_amount = 0;
            staker_info.key = ctx.accounts.staker.key();
            staker_info.staking_pool = staking_pool.key();
        } else {
            let staked_seconds = now.checked_sub(staker_info.last_stake_timestamp as u64).unwrap();
            let rate: f64 = staked_seconds as f64 / staking_pool.total_staked_amount as f64;
            let earned_amount: f64 = rate * staker_info.staked_amount as f64;
    
            staker_info.earned_amount = staker_info.earned_amount.checked_add(earned_amount as u64).unwrap();
        }
        
        staker_info.last_stake_timestamp = now.try_into().unwrap();

        msg!("Staking Pool Total Staked Amount: {:?}", staking_pool.total_staked_amount);
        msg!("Staker Info Amount: {:?}", staker_info.staked_amount);
        msg!("Staker Info Last Stake Timestamp: {:?}", staker_info.last_stake_timestamp);

        Ok(())
    }

    /*
    Unstake a user's tokens. This will transfer the amount of tokens specified by the user + all of the
    accrued unclaimed token rewards from the staking pool vault ATA to the user's wallet. All user information
    and pool information are updated or reset accordingly.
     */
    pub fn unstake (ctx: Context<Unstake>, amount: u64) -> Result<()> {
        msg!("Instruction: Unstake");

        let staking_pool: &mut Account<'_, StakingPool> = &mut ctx.accounts.staking_pool;
        let staker_info: &mut Account<'_, StakerInfo> = &mut ctx.accounts.staker_info;

        require_neq!(staker_info.staked_amount, 0, ErrorCode::NoStake);
        require_gte!(staker_info.staked_amount, amount, ErrorCode::InsufficientStake);

        let now: u64 = Clock::get().unwrap().unix_timestamp.try_into().unwrap();
        let staked_seconds = now.checked_sub(staker_info.last_stake_timestamp as u64).unwrap();
        let rate: f64 = staked_seconds as f64 / staking_pool.total_staked_amount as f64;
        let earned_amount: f64 = rate * staker_info.staked_amount as f64;

        staker_info.earned_amount = staker_info.earned_amount.checked_add(earned_amount as u64).unwrap();

        let total_amount = if staker_info.earned_amount > 0 {
            amount.checked_add(staker_info.earned_amount).unwrap()
        } else {
            amount
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

        staking_pool.total_staked_amount = staking_pool.total_staked_amount.checked_sub(amount).unwrap();
        staking_pool.total_reward_amount = staking_pool.total_reward_amount.checked_sub(staker_info.earned_amount).unwrap();

        //if a user unstakes the full amount, reset the last_stake_timestamp to 0
        if staker_info.staked_amount == amount {
            staker_info.last_stake_timestamp = 0;
        } else {
            staker_info.last_stake_timestamp = now.try_into().unwrap();
        }

        staker_info.staked_amount = staker_info.staked_amount.checked_sub(amount).unwrap();
        staker_info.earned_amount = 0;

        msg!("Staking Pool Total Staked Amount: {:?}", staking_pool.total_staked_amount);
        msg!("Staking Pool Total Reward Amount: {:?}", staking_pool.total_reward_amount);
        msg!("Staker Info Amount: {:?}", staker_info.staked_amount);
        msg!("Staker Info Last Stake Timestamp: {:?}", staker_info.last_stake_timestamp);

        Ok(())
    }

    /*
    Claim a user's vested stake rewards. This will send all the rewards tokens that the user has earned
    to their wallet, but keep their staked tokens in the staking pool. All user information and pool information
    are updated accordingly.
    */
    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        msg!("Instruction: Claim");

        let staking_pool: &mut Account<'_, StakingPool> = &mut ctx.accounts.staking_pool;
        let staker_info: &mut Account<'_, StakerInfo> = &mut ctx.accounts.staker_info;

        let now: u64 = Clock::get().unwrap().unix_timestamp.try_into().unwrap();
        let staked_seconds = now.checked_sub(staker_info.last_stake_timestamp as u64).unwrap();
        let rate: f64 = staked_seconds as f64 / staking_pool.total_staked_amount as f64;
        let earned_amount: f64 = rate * staker_info.staked_amount as f64;

        staker_info.earned_amount = staker_info.earned_amount.checked_add(earned_amount as u64).unwrap();
        staker_info.last_stake_timestamp = now.try_into().unwrap();

        msg!("Staker Info Earned Amount: {:?}", staker_info.earned_amount);

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
            staker_info.earned_amount,
        )?;

        staking_pool.total_reward_amount = staking_pool.total_reward_amount.checked_sub(staker_info.earned_amount).unwrap();
        staker_info.earned_amount = 0;

        msg!("Staking Pool Total Reward Amount: {:?}", staking_pool.total_reward_amount);

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init_if_needed,
        seeds=[b"pool", owner.key().as_ref()], 
        bump,
        payer = owner,
        space = 8 + StakingPool::INIT_SPACE)]
    pub staking_pool: Account<'info, StakingPool>,

    pub stake_token_mint: Account<'info, Mint>,

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
    pub vault_ata: Account<'info, TokenAccount>,

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

    pub stake_token_mint: Account<'info, Mint>,

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
        seeds=[b"stake", staker.key().as_ref(), staking_pool.owner.key().as_ref()],
        bump,
        payer = staker, 
        space = 8 + StakerInfo::INIT_SPACE)]
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

#[derive(Accounts)]
pub struct Claim<'info> {
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
#[derive(InitSpace)]
#[derive(Default)]
pub struct StakerInfo {
    pub key: Pubkey,
    pub staked_amount: u64,
    pub earned_amount: u64,
    pub last_stake_timestamp: i64,
    pub staking_pool: Pubkey,
}

#[account]
#[derive(InitSpace)]
#[derive(Default)]
pub struct StakingPool {
    pub owner: Pubkey,
    pub staking_token_mint: Pubkey,
    pub total_staked_amount: u64,
    pub total_reward_amount: u64,
    pub reward_per_second: u64,
    pub bump: u8,
}

#[error_code]
pub enum ErrorCode {
    #[msg("You may only unstake the amount that you have staked")]
    InsufficientStake,
    #[msg("You must stake more than 0 tokens")]
    ZeroStake,
    #[msg("You are not currently staking any tokens")]
    NoStake,
}
