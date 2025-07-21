use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct UserAccount {
    // user points
    pub points: u32,
    // The number of NFTs staked by the user
    pub amount_staked: u8,
    pub bump: u8,
}

// user have many stake accounts
