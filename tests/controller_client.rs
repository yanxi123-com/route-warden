use serde_json::json;
use tempfile::Builder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
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
            "now": "NodeA",
            "all": ["NodeA", "NodeB"]
        })))
        .mount(&server)
        .await;

    let proxies = client.list_proxies().await.unwrap();
    assert!(proxies.contains(&"GLOBAL".to_string()));
    assert!(proxies.contains(&"NodeA".to_string()));

    let members = client.get_group_members("GLOBAL").await.unwrap();
    assert_eq!(members, vec!["NodeA".to_string(), "NodeB".to_string()]);

    let current = client.get_group_current("GLOBAL").await.unwrap();
    assert_eq!(current, "NodeA".to_string());
}

#[tokio::test]
async fn list_proxies_over_unix_socket() {
    let dir = Builder::new().prefix("rw-").tempdir_in("/tmp").unwrap();
    let socket_path = dir.path().join("mihomo.sock");
    let listener = UnixListener::bind(&socket_path).unwrap();

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request_buf = [0_u8; 4096];
        let read = stream.read(&mut request_buf).await.unwrap();
        let request_text = String::from_utf8_lossy(&request_buf[..read]);
        assert!(request_text.starts_with("GET /proxies "));

        let body = r#"{"proxies":{"GLOBAL":{},"NodeA":{}}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).await.unwrap();
    });

    let base_url = format!("unix://{}", socket_path.display());
    let client = route_warden::controller::ControllerClient::new(&base_url, None).unwrap();
    let proxies = client.list_proxies().await.unwrap();

    assert!(proxies.contains(&"GLOBAL".to_string()));
    assert!(proxies.contains(&"NodeA".to_string()));
    server.await.unwrap();
}
