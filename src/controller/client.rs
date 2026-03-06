use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ControllerClient {
    request_base: String,
    secret: Option<String>,
    http: Client,
}

impl ControllerClient {
    pub fn new(base_url: &str, secret: Option<String>) -> Result<Self> {
        let raw = base_url.trim();
        if raw.is_empty() {
            bail!("controller base_url 不能为空");
        }

        let (request_base, http) = if let Some(path) = raw.strip_prefix("unix://") {
            #[cfg(unix)]
            {
                let socket_path = path.trim();
                if socket_path.is_empty() {
                    bail!("unix socket 路径不能为空");
                }
                let client = Client::builder()
                    .unix_socket(socket_path)
                    .build()
                    .context("创建 unix socket controller 客户端失败")?;
                ("http://localhost".to_string(), client)
            }
            #[cfg(not(unix))]
            {
                let _ = path;
                bail!("当前平台不支持 unix:// controller");
            }
        } else {
            (raw.trim_end_matches('/').to_string(), Client::new())
        };

        if request_base.is_empty() {
            bail!("controller base_url 不能为空");
        }

        Ok(Self {
            request_base,
            secret,
            http,
        })
    }

    pub async fn list_proxies(&self) -> Result<Vec<String>> {
        let value = self.get_json("/proxies").await?;
        let proxies = value
            .get("proxies")
            .and_then(Value::as_object)
            .context("响应缺少 proxies 对象")?;

        Ok(proxies.keys().cloned().collect())
    }

    pub async fn get_group_members(&self, group: &str) -> Result<Vec<String>> {
        let path = format!("/proxies/{group}");
        let value = self.get_json(&path).await?;

        let all = value
            .get("all")
            .and_then(Value::as_array)
            .context("响应缺少 all 字段")?;

        let members = all
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect();
        Ok(members)
    }

    pub async fn get_group_current(&self, group: &str) -> Result<String> {
        let path = format!("/proxies/{group}");
        let value = self.get_json(&path).await?;
        let now = value
            .get("now")
            .and_then(Value::as_str)
            .context("响应缺少 now 字段")?;
        Ok(now.to_string())
    }

    pub async fn switch_group(&self, group: &str, node: &str) -> Result<()> {
        if group.trim().is_empty() {
            bail!("group 不能为空");
        }
        if node.trim().is_empty() {
            bail!("node 不能为空");
        }

        let url = format!("{}/proxies/{}", self.request_base, group);
        let mut req = self
            .http
            .put(url)
            .json(&serde_json::json!({ "name": node }));
        if let Some(secret) = &self.secret {
            req = req.bearer_auth(secret);
        }

        let response = req.send().await.context("请求切换策略组失败")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("切换策略组失败: status={status}, body={body}");
        }
        Ok(())
    }

    async fn get_json(&self, path: &str) -> Result<Value> {
        let url = format!("{}{}", self.request_base, path);
        let mut req = self.http.get(url);
        if let Some(secret) = &self.secret {
            req = req.bearer_auth(secret);
        }
        let response = req.send().await.context("请求 controller 失败")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("controller 返回错误: status={status}, body={body}");
        }
        response
            .json()
            .await
            .context("解析 controller 响应 JSON 失败")
    }
}
