use cosmwasm_std::StdError;
use thiserror::Error;

/// ## Description
/// This enum describes airdrop contract errors
#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Already claimed")]
    AlreadyClaimed {},

    #[error("Invalid merkle root")]
    InvalidMerkle {},

    #[error("Merkle verification failed")]
    MerkleVerification {},

    #[error("Unauthorized")]
    Unauthorized {},
}
