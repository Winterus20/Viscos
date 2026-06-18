//! REST integration test (mockito).
//!
//! Twilight HTTP client 0.17'de `proxy_url` desteği kaldırıldığı için
//! gerçek mock server'a yönlendirme v1'de mümkün değil. Bu test
//! `ViscosHttp` builder akışının sağlıklı kurulabildiğini doğrular
//! (token audit, builder, expose). `twilight_http::Client::builder()` artık
//! tokio runtime gerektirdiğinden testler `#[tokio::test]` ile çalışır.
//!
//! Faz 2.0+ follow-up'ta `twilight_http::Client::builder()` `proxy_url`'ü
//! geri aldığında, `#[ignore]` edilen test mockito ile çalıştırılabilir.

use secrecy::SecretString;
use viscos_api::ViscosHttp;

#[tokio::test]
async fn http_client_can_be_built_with_token() {
    let vh = ViscosHttp::new(SecretString::new("test-token".to_string().into_boxed_str()))
        .expect("build");
    assert_eq!(vh.expose_token(), "test-token");
}

#[tokio::test]
async fn http_client_builder_timeout_override() {
    use std::time::Duration;
    let vh = ViscosHttp::builder(SecretString::new("test".to_string().into_boxed_str()))
        .timeout(Duration::from_secs(5))
        .build()
        .expect("build");
    assert_eq!(vh.expose_token(), "test");
}

#[tokio::test]
async fn expose_token_round_trip_preserves_value() {
    // SecretString sıfırdan üretilip expose edilince aynı string'i vermeli.
    let vh = ViscosHttp::new(SecretString::new(
        "super-secret-12345".to_string().into_boxed_str(),
    ))
    .expect("build");
    assert_eq!(vh.expose_token(), "super-secret-12345");
}
