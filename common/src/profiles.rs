use std::collections::BTreeMap;

use crate::{
    charge_fees_to_account_no_bump_reputation, reward_e9s_per_block, slice_to_32_bytes_array,
    zlib_decompress, DccIdentity, LABEL_NP_PROFILE, LABEL_NP_REGISTER,
};
use candid::Principal;
#[cfg(target_arch = "wasm32")]
#[allow(unused_imports)]
use ic_cdk::println;
use ledger_map::LedgerMap;
use serde::{Deserialize, Serialize};

pub fn operation_fee_e9s() -> u64 {
    reward_e9s_per_block() / 10000
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct NodeProviderProfile {
    pub name: String,
    pub description: String,
    pub url: String,
    pub logo_url: String,
    pub why_choose_us: String,
    pub locations: BTreeMap<String, String>,
    pub contacts: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct NodeProviderProfileWithReputation {
    pub profile: NodeProviderProfile,
    pub reputation: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct UpdateProfilePayload {
    pub profile_payload: Vec<u8>,
    pub signature: Vec<u8>,
}

pub fn do_node_provider_update_profile(
    ledger: &mut LedgerMap,
    caller: Principal,
    np_uid_bytes: Vec<u8>,
    update_profile_payload: Vec<u8>,
) -> Result<String, String> {
    if np_uid_bytes.len() > 64 {
        return Err("Node provider unique id too long".to_string());
    }

    let dcc_identity =
        DccIdentity::new_verifying_from_bytes(&np_uid_bytes).map_err(|e| e.to_string())?;
    if caller != dcc_identity.to_ic_principal() {
        return Err("Invalid caller".to_string());
    }
    println!("[do_node_provider_update_profile]: {}", dcc_identity);

    let payload: UpdateProfilePayload =
        serde_json::from_slice(&update_profile_payload).map_err(|e| e.to_string())?;

    if payload.signature.len() != 64 {
        return Err("Invalid signature".to_string());
    }
    if payload.profile_payload.len() > 1024 {
        return Err("Profile payload too long".to_string());
    }

    match dcc_identity.verify_bytes(&payload.profile_payload, &payload.signature) {
        Ok(()) => {
            charge_fees_to_account_no_bump_reputation(
                ledger,
                &dcc_identity,
                np_profile_update_fee_e9s(),
            )?;
            // Store the original signed payload in the ledger
            ledger
                .upsert(LABEL_NP_PROFILE, &np_uid_bytes, &update_profile_payload)
                .map(|_| "Profile updated! Thank you.".to_string())
                .map_err(|e| e.to_string())
        }
        Err(e) => Err(format!("Signature is invalid: {:?}", e)),
    }
}

pub fn do_node_provider_get_profile(ledger: &LedgerMap, np_uid_bytes: Vec<u8>) -> Option<String> {
    match ledger.get(LABEL_NP_PROFILE, &np_uid_bytes) {
        Ok(profile) => {
            let payload: UpdateProfilePayload =
                serde_json::from_slice(&profile).expect("Failed to decode profile payload");
            let profile: NodeProviderProfile = serde_json::from_str(
                &zlib_decompress(&payload.profile_payload).expect("Failed to decompress profile"),
            )
            .expect("Failed to decode profile");
            let profile_with_reputation = NodeProviderProfileWithReputation {
                profile,
                reputation: reputation_get(&np_uid_bytes),
            };
            Some(serde_json::to_string(&profile_with_reputation).expect("Failed to encode profile"))
        }
        Err(_) => None,
    }
}
