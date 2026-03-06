use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn switch_proxy_group_node() {
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/proxies/GLOBAL"))
        .and(body_json(json!({ "name": "NodeA" })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let result = route_warden::controller::switch_group(&server.uri(), "GLOBAL", "NodeA").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn list_proxies_and_get_group_members() {
    let server = MockServer::start().await;
    let client = route_warden::controller::ControllerClient::new(&server.uri(), None).unwrap();

    Mock::given(method("GET"))
        .and(path("/proxies"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "proxies": {
                "GLOBAL": {},
                "NodeA": {},
                "NodeB": {}
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/proxies/GLOBAL"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "GLOBAL",
            "all": ["NodeA", "NodeB"]
        })))
        .mount(&server)
        .await;

    let proxies = client.list_proxies().await.unwrap();
    assert!(proxies.contains(&"GLOBAL".to_string()));
    assert!(proxies.contains(&"NodeA".to_string()));

    let members = client.get_group_members("GLOBAL").await.unwrap();
    assert_eq!(members, vec!["NodeA".to_string(), "NodeB".to_string()]);
}
