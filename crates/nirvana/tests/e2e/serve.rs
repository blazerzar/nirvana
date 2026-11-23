use crate::helpers::Nirvana;

#[tokio::test]
#[ignore]
async fn serves_web_and_api() {
    let nirvana = Nirvana::new();
    let server = nirvana.serve();
    let url = format!("http://127.0.0.1:{}", server.port);

    let home = reqwest::get(&url).await.unwrap();
    assert!(home.status().is_success(), "{}", home.status());
    let html = home.text().await.unwrap();
    assert!(html.contains("nirvana"), "{html}");

    let info: serde_json::Value = reqwest::get(format!("{url}/api/info"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
}
