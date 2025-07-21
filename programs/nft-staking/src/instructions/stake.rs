use anchor_lang::prelude::*;
use anchor_spl::{
    metadata::{
        mpl_token_metadata::instructions::{
            FreezeDelegatedAccountCpi, FreezeDelegatedAccountCpiAccounts,
        },
        MasterEditionAccount, Metadata, MetadataAccount,
    },
    token::{approve, Approve, Mint, Token, TokenAccount},
};

use crate::{
    error::StakeError,
    state::{StakeAccount, StakeConfig, UserAccount},
};

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    // The mint of the NFT being staked
    pub mint: Account<'info, Mint>,
    // The collection mint of the NFT
    pub collection_mint: Account<'info, Mint>,
    // nft's associated token account
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = user,
    )]
    pub mint_ata: Account<'info, TokenAccount>,
    // nft metadata
    #[account(
        seeds = [
            b"metadata",
            metadata_program.key().as_ref(),
            mint.key().as_ref()
        ],
        seeds::program = metadata_program.key(),
        bump,
        // Ensure the metadata is for the correct collection and is verified
        constraint = metadata.collection.as_ref().unwrap().key.as_ref() == collection_mint.key().as_ref(),
        // Ensure the collection is verified
        constraint = metadata.collection.as_ref().unwrap().verified == true,
    )]
    pub metadata: Account<'info, MetadataAccount>,
    // master edition account
    #[account(
        // each NFT has a unique edition account
        seeds = [
            b"metadata",
            metadata_program.key().as_ref(),
            mint.key().as_ref(),
            b"edition"
        ],
        seeds::program = metadata_program.key(),
        bump,
    )]
    pub edition: Account<'info, MasterEditionAccount>,
    #[account(
        seeds = [b"config".as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, StakeConfig>,
    #[account(
        init,
        payer = user,
        space = 8 + StakeAccount::INIT_SPACE,
        seeds = [b"stake".as_ref(), mint.key().as_ref(), config.key().as_ref()],
        bump,
    )]
    pub stake_account: Account<'info, StakeAccount>,
    #[account(
        mut,
        seeds = [b"user".as_ref(), user.key().as_ref()],
        bump = user_account.bump,
    )]
    pub user_account: Account<'info, UserAccount>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub metadata_program: Program<'info, Metadata>,
}

impl<'info> Stake<'info> {
    pub fn stake(&mut self, bumps: &StakeBumps) -> Result<()> {
        require!(
            self.user_account.amount_staked < self.config.max_stake,
            StakeError::MaxStakeReached
        );

        // set the stake account with the current user, nft mint, and timestamp
        self.stake_account.set_inner(StakeAccount {
            owner: self.user.key(),
            mint: self.mint.key(),
            staked_at: Clock::get()?.unix_timestamp,
            bump: bumps.stake_account,
        });

        let cpi_program = self.token_program.to_account_info();

        let cpi_accounts = Approve {
            // user's associated token account
            to: self.mint_ata.to_account_info(),
            // the stake account that will receive the delegated authority of the NFT
            delegate: self.stake_account.to_account_info(),
            authority: self.user.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        approve(cpi_ctx, 1)?;

        // stake account seeds for the FreezeDelegatedAccountCpi
        // The seeds for the stake account are the mint and config keys, along with the bump
        let seeds = &[
            b"stake",
            self.mint.to_account_info().key.as_ref(),
            self.config.to_account_info().key.as_ref(),
            &[self.stake_account.bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let delegate = &self.stake_account.to_account_info();
        let token_account = &self.mint_ata.to_account_info();
        let edition = &self.edition.to_account_info();
        let mint = &self.mint.to_account_info();
        let token_program = &self.token_program.to_account_info();
        let metadata_program = &self.metadata_program.to_account_info();

        // https://docs.rs/mpl-token-metadata/latest/mpl_token_metadata/instructions/struct.FreezeDelegatedAccountCpi.html
        // Freeze the NFT by using the delegated authority (stake_account)
        // staking staus is set by freezing the NFT
        // lock the NFT so it cannot be transferred or burned
        FreezeDelegatedAccountCpi::new(
            metadata_program,
            FreezeDelegatedAccountCpiAccounts {
                delegate,      // stake_account (the account that will freeze the NFT)
                token_account, // user's NFT token account
                edition,       // NFT's Master Edition
                mint,          // NFT's Mint
                token_program, // SPL Token Program
            },
        )
        .invoke_signed(signer_seeds)?; // Invoke the CPI to freeze the NFT by using the delegated authority(stake_account)

        self.user_account.amount_staked += 1;

        Ok(())
        // nft is staked but the user still owns it
        // user's NFT token account is frozen
        // stake_account has the authority to manage the NFT
        // during the stake period, the user cannot transfer or burn the NFT
    }
}
