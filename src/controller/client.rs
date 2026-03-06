use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ControllerClient {
    base_url: String,
    secret: Option<String>,
    http: Client,
}

impl ControllerClient {
    pub fn new(base_url: &str, secret: Option<String>) -> Result<Self> {
        let base = base_url.trim_end_matches('/').to_string();
        if base.is_empty() {
            bail!("controller base_url 不能为空");
        }

        Ok(Self {
            base_url: base,
            secret,
            http: Client::new(),
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

    pub async fn switch_group(&self, group: &str, node: &str) -> Result<()> {
        if group.trim().is_empty() {
            bail!("group 不能为空");
        }
        if node.trim().is_empty() {
            bail!("node 不能为空");
        }

        let url = format!("{}/proxies/{}", self.base_url, group);
        let mut req = self.http.put(url).json(&serde_json::json!({ "name": node }));
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
        let url = format!("{}{}", self.base_url, path);
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
        response.json().await.context("解析 controller 响应 JSON 失败")
    }
}
