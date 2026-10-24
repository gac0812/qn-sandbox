//! Opt-in version contract against an explicitly supplied pinned release binary.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[tokio::test]
#[ignore = "requires QN_TEST_NONO pointing to a checksum-verified nono 0.80.0 binary"]
async fn it_verifies_the_real_pinned_binary() {
    let binary = std::env::var("QN_TEST_NONO").expect("set QN_TEST_NONO to the absolute nono path");
    qn_nono::verify_version(camino::Utf8Path::new(&binary))
        .await
        .expect("the verified release must report the pinned version");
}
