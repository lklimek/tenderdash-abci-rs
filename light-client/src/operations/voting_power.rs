use crate::{
    bail,
    predicates::errors::VerificationError,
    types::{Commit, SignedHeader, TrustThreshold, ValidatorSet},
};

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

use tendermint::block::CommitSig;
use tendermint::lite::types::TrustThreshold as _;
use tendermint::lite::types::ValidatorSet as _;
use tendermint::vote::{SignedVote, Vote};

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VotingPowerTally {
    pub total: u64,
    pub tallied: u64,
    pub trust_threshold: TrustThreshold,
}

impl fmt::Display for VotingPowerTally {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "VotingPower(total={} tallied={} trust_threshold={})",
            self.total, self.tallied, self.trust_threshold
        )
    }
}

pub trait VotingPowerCalculator: Send {
    fn total_power_of(&self, validators: &ValidatorSet) -> u64;

    fn check_enough_trust(
        &self,
        untrusted_header: &SignedHeader,
        untrusted_validators: &ValidatorSet,
        trust_threshold: TrustThreshold,
    ) -> Result<(), VerificationError> {
        let voting_power =
            self.voting_power_in(untrusted_header, untrusted_validators, trust_threshold)?;

        if trust_threshold.is_enough_power(voting_power.tallied, voting_power.total) {
            Ok(())
        } else {
            Err(VerificationError::NotEnoughTrust(voting_power))
        }
    }

    fn check_signers_overlap(
        &self,
        untrusted_header: &SignedHeader,
        untrusted_validators: &ValidatorSet,
    ) -> Result<(), VerificationError> {
        let trust_threshold = TrustThreshold::TWO_THIRDS;
        let voting_power =
            self.voting_power_in(untrusted_header, untrusted_validators, trust_threshold)?;

        if trust_threshold.is_enough_power(voting_power.tallied, voting_power.total) {
            Ok(())
        } else {
            Err(VerificationError::InsufficientSignersOverlap(voting_power))
        }
    }

    fn voting_power_in(
        &self,
        signed_header: &SignedHeader,
        validator_set: &ValidatorSet,
        trust_threshold: TrustThreshold,
    ) -> Result<VotingPowerTally, VerificationError>;
}

#[derive(Copy, Clone, Debug)]
pub struct ProdVotingPowerCalculator;

impl VotingPowerCalculator for ProdVotingPowerCalculator {
    fn total_power_of(&self, validators: &ValidatorSet) -> u64 {
        validators.total_power()
    }

    fn voting_power_in(
        &self,
        signed_header: &SignedHeader,
        validator_set: &ValidatorSet,
        trust_threshold: TrustThreshold,
    ) -> Result<VotingPowerTally, VerificationError> {
        let signatures = &signed_header.commit.signatures;

        let mut tallied_voting_power = 0_u64;
        let mut seen_validators = HashSet::new();

        // Get non-absent votes from the signatures
        let non_absent_votes = signatures.iter().enumerate().flat_map(|(idx, signature)| {
            if let Some(vote) = non_absent_vote(signature, idx as u64, &signed_header.commit) {
                Some((signature, vote))
            } else {
                None
            }
        });

        for (signature, vote) in non_absent_votes {
            // Ensure we only count a validator's power once
            if seen_validators.contains(&vote.validator_address) {
                bail!(VerificationError::DuplicateValidator(
                    vote.validator_address
                ));
            } else {
                seen_validators.insert(vote.validator_address);
            }

            let validator = match validator_set.validator(vote.validator_address) {
                Some(validator) => validator,
                None => continue, // Cannot find matching validator, so we skip the vote
            };

            let signed_vote = SignedVote::new(
                (&vote).into(),
                signed_header.header.chain_id.as_str(),
                vote.validator_address,
                vote.signature,
            );

            // Check vote is valid
            let sign_bytes = signed_vote.sign_bytes();
            if !validator.verify_signature(&sign_bytes, signed_vote.signature()) {
                bail!(VerificationError::InvalidSignature {
                    signature: signed_vote.signature().to_vec(),
                    validator,
                    sign_bytes,
                });
            }

            // If the vote is neither absent nor nil, tally its power
            if signature.is_commit() {
                tallied_voting_power += validator.power();
            } else {
                // It's OK. We include stray signatures (~votes for nil)
                // to measure validator availability.
            }

            // TODO: Break out of the loop when we have enough voting power.
            // See https://github.com/informalsystems/tendermint-rs/issues/235
        }

        let voting_power = VotingPowerTally {
            total: self.total_power_of(validator_set),
            tallied: tallied_voting_power,
            trust_threshold,
        };

        Ok(voting_power)
    }
}

fn non_absent_vote(commit_sig: &CommitSig, validator_index: u64, commit: &Commit) -> Option<Vote> {
    let (validator_address, timestamp, signature, block_id) = match commit_sig {
        CommitSig::BlockIDFlagAbsent { .. } => return None,
        CommitSig::BlockIDFlagCommit {
            validator_address,
            timestamp,
            signature,
        } => (
            *validator_address,
            *timestamp,
            signature.clone(),
            Some(commit.block_id.clone()),
        ),
        CommitSig::BlockIDFlagNil {
            validator_address,
            timestamp,
            signature,
        } => (*validator_address, *timestamp, signature.clone(), None),
    };

    Some(Vote {
        vote_type: tendermint::vote::Type::Precommit,
        height: commit.height,
        round: commit.round,
        block_id,
        timestamp,
        validator_address,
        validator_index,
        signature,
    })
}
