use base64::Engine;
use rand::Rng;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct PkceSession {
    pub code_verifier: String,
    pub code_challenge: String,
    pub state: String,
}

fn generate_random_verifier() -> String {
    let mut rng = rand::thread_rng();
    let mut verifier_bytes = [0u8; 32];
    rng.fill(&mut verifier_bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(verifier_bytes)
}

fn generate_random_state() -> String {
    let mut rng = rand::thread_rng();
    let mut state_bytes = [0u8; 16];
    rng.fill(&mut state_bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(state_bytes)
}

pub fn generate_pkce() -> PkceSession {
    #[cfg(debug_assertions)]
    let code_verifier = if let Ok(v) = std::env::var("OPI_TEST_VERIFIER") {
        v
    } else {
        generate_random_verifier()
    };

    #[cfg(not(debug_assertions))]
    let code_verifier = generate_random_verifier();

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);

    #[cfg(debug_assertions)]
    let state = if let Ok(s) = std::env::var("OPI_TEST_STATE") {
        s
    } else {
        generate_random_state()
    };

    #[cfg(not(debug_assertions))]
    let state = generate_random_state();

    PkceSession {
        code_verifier,
        code_challenge,
        state,
    }
}
