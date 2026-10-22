//! The updater must only offer newer releases, and must refuse any download
//! whose signature does not match — a wrong answer here installs untrusted code.

use base64::Engine as _;

use super::{newer_update, verify, Artifact, Manifest};

fn manifest(version: &str) -> Manifest {
    Manifest {
        version: version.to_string(),
        platforms: [(
            "linux-x86_64".to_string(),
            Artifact {
                signature: "sig".to_string(),
                url: "https://example.invalid/app.AppImage".to_string(),
            },
        )]
        .into(),
    }
}

/// A throwaway keypair, encoded as the release tooling encodes the real one.
fn signer() -> (minisign::SecretKey, String) {
    let keys = minisign::KeyPair::generate_unencrypted_keypair().expect("keypair");
    let public = keys.pk.to_box().expect("public key box").to_string();
    let public = base64::engine::general_purpose::STANDARD.encode(public);
    (keys.sk, public)
}

fn sign(secret: &minisign::SecretKey, bytes: &[u8]) -> String {
    let signature =
        minisign::sign(None, secret, bytes, Some("trusted"), Some("untrusted")).expect("sign");
    base64::engine::general_purpose::STANDARD.encode(signature.to_string())
}

#[test]
fn only_a_strictly_newer_release_is_offered() {
    let offered = newer_update(&manifest("1.19.0"), "1.18.1", "linux-x86_64").expect("valid");
    assert_eq!(offered.map(|u| u.version), Some("1.19.0".to_string()));

    // The same version, or an older one republished, must not loop the user
    // through an install of what they already run.
    assert!(newer_update(&manifest("1.18.1"), "1.18.1", "linux-x86_64")
        .expect("valid")
        .is_none());
    assert!(newer_update(&manifest("1.9.9"), "1.18.1", "linux-x86_64")
        .expect("valid")
        .is_none());
}

#[test]
fn a_tag_style_version_and_a_missing_platform_are_handled() {
    // Releases are tagged `v1.2.3`; the feed sometimes carries the prefix.
    assert!(newer_update(&manifest("v2.0.0"), "1.18.1", "linux-x86_64")
        .expect("valid")
        .is_some());
    // A release without this platform's build is an error, not a silent "up to date".
    assert!(newer_update(&manifest("2.0.0"), "1.18.1", "windows-x86_64").is_err());
}

#[test]
fn a_correctly_signed_download_verifies() {
    let (secret, public) = signer();
    let payload = b"installer bytes";
    assert!(verify(payload, &sign(&secret, payload), &public).is_ok());
}

#[test]
fn tampered_bytes_or_a_foreign_key_are_rejected() {
    let (secret, public) = signer();
    let signature = sign(&secret, b"installer bytes");
    assert!(verify(b"installer bytez", &signature, &public).is_err());

    let (_, other_public) = signer();
    assert!(verify(b"installer bytes", &signature, &other_public).is_err());
}
