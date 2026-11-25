use std::sync::LazyLock;
use std::time::Duration;

pub const USER_AGENT: &str = "moonlauncher/27.0.1";

pub static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(6))
        // Detecta sockets rotos tras cambiar de red/VPN y evita reutilizarlos.
        .tcp_keepalive(Duration::from_secs(5))
        .pool_idle_timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(16)
        .build()
        .expect("Failed to create HTTP client")
});
