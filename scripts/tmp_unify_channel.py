import io

p = 'crates/cloudmount-daemon/src/main.rs'
s = io.open(p, encoding='utf-8').read()

# 1. async_main 的 provider 构造改为通道选择
old = '''    // ---- 同步模式（默认）----
    let app_id = cli.app_id.clone().ok_or_else(|| {
        anyhow::anyhow!(
            "缺少 AppID：请设置 CLOUDMOUNT_APP_ID 或传 --app-id（申请入口 https://open.115.com/）"
        )
    })?;
    let store = cloudmount_auth::store::default_store()?;
    let auth = Arc::new(OpenApiAuth::new(app_id, store));
    if !auth.is_logged_in() {
        anyhow::bail!("未登录：请先执行 `cloudmount login` 完成扫码授权");
    }
    let provider: Arc<dyn Provider> = Arc::new(Open115Provider::new(auth));'''
new = '''    // ---- 同步模式（默认）----
    let provider = build_channel_provider(&cli)?;'''
assert old in s, "async"
s = s.replace(old, new)

# 2. Windows build_provider 的通道段抽成跨平台函数
old = '''    // Cookie 补充通道（显式 opt-in；知情同意已在 login 时完成）
    if cli.channel == "cookie" {
        tracing::warn!(
            "使用 Cookie 补充通道（违反 115 服务协议）。\\
             失效探测只读不踢线，失效时请重新扫码。"
        );
        let policy = cloudmount_auth::cookie::CookiePolicy {
            enabled: true,
            consent_acknowledged: true,
        };
        let store = cloudmount_auth::store::default_store()?;
        let auth = CookieAuth::new(policy, store);
        let provider = cloudmount_driver115::Cookie115Provider::try_new(policy, auth)?;
        return Ok(Arc::new(provider));
    }

    // auto：官方为主，Cookie 兜底（官方认证失效时无感切换，可见提示）
    if cli.channel == "auto" {
        if let Some(cookie) = try_cookie_provider() {
            let open = build_open_provider(&app_id)?;
            tracing::info!("双通道路由就绪：官方为主，Cookie 兜底（认证失效时自动切换）");
            return Ok(Arc::new(cloudmount_driver115::ProviderRouter::new(
                Box::new(open),
                Some(Box::new(cookie)),
            )));
        }
    }

    build_provider_from_app_id(&app_id)
}'''
new = '''    build_channel_provider(cli)
}

/// 按通道选择构建 Provider（open/cookie/auto；跨平台，同步与挂载共用）。
///
/// - `cookie`：仅补充通道（显式 opt-in；知情同意已在 login 时完成）；
/// - `auto`（默认）：官方为主，Cookie 兜底——官方**认证失效**时无感
///   切换（tracing::warn 可见 + `last_switch` 供 UI 常驻提示）；
/// - `open`：仅官方。
fn build_channel_provider(cli: &Cli) -> anyhow::Result<Arc<dyn Provider>> {
    if cli.channel == "cookie" {
        tracing::warn!(
            "使用 Cookie 补充通道（违反 115 服务协议）。\\
             失效探测只读不踢线，失效时请重新扫码。"
        );
        let policy = cloudmount_auth::cookie::CookiePolicy {
            enabled: true,
            consent_acknowledged: true,
        };
        let store = cloudmount_auth::store::default_store()?;
        let auth = CookieAuth::new(policy, store);
        let provider = cloudmount_driver115::Cookie115Provider::try_new(policy, auth)?;
        return Ok(Arc::new(provider));
    }

    let app_id = cli
        .app_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("缺少 AppID：请设置 CLOUDMOUNT_APP_ID 或传 --app-id"))?;

    // auto：官方为主，Cookie 兜底
    if cli.channel == "auto" {
        if let Some(cookie) = try_cookie_provider() {
            let open = build_open_provider(&app_id)?;
            tracing::info!("双通道路由就绪：官方为主，Cookie 兜底（认证失效时自动切换）");
            return Ok(Arc::new(cloudmount_driver115::ProviderRouter::new(
                Box::new(open),
                Some(Box::new(cookie)),
            )));
        }
    }

    build_provider_from_app_id(&app_id)
}'''
assert old in s, "channel"
s = s.replace(old, new)

# 3. 去掉 cfg(windows) 门控（跨平台）
s = s.replace('''#[cfg(windows)]
fn try_cookie_provider()''', '''fn try_cookie_provider()''')
s = s.replace('''#[cfg(windows)]
use cloudmount_auth::cookie::CookieAuth;''', '''use cloudmount_auth::cookie::CookieAuth;''')

io.open(p, 'w', encoding='utf-8').write(s)
print('ok')
