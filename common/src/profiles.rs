use crate::{
    charge_fees_to_account_no_bump_reputation, info, reputation_get, reward_e9s_per_block, Balance,
    DccIdentity, ED25519_SIGNATURE_LENGTH, LABEL_NP_PROFILE, MAX_NP_PROFILE_BYTES,
    MAX_PUBKEY_BYTES,
};
use candid::Principal;
#[cfg(target_arch = "wasm32")]
#[allow(unused_imports)]
use ic_cdk::println;
use ledger_map::LedgerMap;
use serde::{Deserialize, Serialize};

pub fn np_profile_update_fee_e9s() -> Balance {
    reward_e9s_per_block() / 10000
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct UpdateProfilePayload {
    pub profile_payload: Vec<u8>,
    pub signature: Vec<u8>,
}

pub fn do_node_provider_update_profile(
    ledger: &mut LedgerMap,
    caller: Principal,
    pubkey_bytes: Vec<u8>,
    update_profile_payload: &[u8],
) -> Result<String, String> {
    if pubkey_bytes.len() > MAX_PUBKEY_BYTES {
        return Err("Provided public key too long".to_string());
    }

    let dcc_identity =
        DccIdentity::new_verifying_from_bytes(&pubkey_bytes).map_err(|e| e.to_string())?;
    if caller != dcc_identity.to_ic_principal() {
        return Err("Invalid caller".to_string());
    }
    info!("[do_node_provider_update_profile]: {}", dcc_identity);

    let payload: UpdateProfilePayload =
        serde_json::from_slice(update_profile_payload).map_err(|e| e.to_string())?;

    if payload.signature.len() != ED25519_SIGNATURE_LENGTH {
        return Err("Invalid signature".to_string());
    }
    if payload.profile_payload.len() > MAX_NP_PROFILE_BYTES {
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
                .upsert(LABEL_NP_PROFILE, &pubkey_bytes, update_profile_payload)
                .map(|_| "Profile updated! Thank you.".to_string())
                .map_err(|e| e.to_string())
        }
        Err(e) => Err(format!("Signature is invalid: {:?}", e)),
    }
}

#[derive(Serialize)]
pub struct NodeProviderProfileWithReputation {
    pub profile: np_profile::Profile,
    pub reputation: u64,
}

pub fn do_node_provider_get_profile(
    ledger: &LedgerMap,
    pubkey_bytes: Vec<u8>,
) -> Option<NodeProviderProfileWithReputation> {
    match ledger.get(LABEL_NP_PROFILE, &pubkey_bytes) {
        Ok(profile_bytes) => {
            let payload: UpdateProfilePayload =
                serde_json::from_slice(&profile_bytes).expect("Failed to decode profile payload");
            let profile_str =
                std::str::from_utf8(&payload.profile_payload).expect("Failed to decode profile");
            let profile = np_profile::Profile::new_from_str(profile_str, "json")
                .expect("Failed to decode profile");
            Some(NodeProviderProfileWithReputation {
                profile,
                reputation: reputation_get(&pubkey_bytes),
            })
        }
        Err(_) => None,
    }
}
