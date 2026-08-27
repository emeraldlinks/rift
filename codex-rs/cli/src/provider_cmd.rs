use std::collections::HashMap;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_core::config::find_codex_home;
use codex_model_provider_info::BUILT_IN_PROVIDER_IDS;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::WireApi;
use codex_utils_cli::CliConfigOverrides;

use crate::cloud_config;

/// Subcommands:
/// - `list`   — list all providers (built-in + user-configured)
/// - `add`    — add a custom provider to ~/.codex/config.toml
/// - `remove` — remove a user-configured provider
/// - `test`   — test provider connectivity with a simple request
/// - `status` — check connectivity to all configured providers at once
#[derive(Debug, clap::Parser)]
pub struct ProviderCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: ProviderSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum ProviderSubcommand {
    List(ListArgs),
    Add(AddArgs),
    Edit(EditArgs),
    Remove(RemoveArgs),
    Models(ModelsArgs),
    Test(TestArgs),
    Login(LoginArgs),
    Status(StatusArgs),
}

#[derive(Debug, clap::Parser)]
pub struct ListArgs {
    /// Output providers as JSON.
    #[arg(long)]
    pub json: bool,

    /// Show only user-configured providers (exclude built-in).
    #[arg(long)]
    pub custom_only: bool,
}

#[derive(Debug, clap::Parser)]
pub struct AddArgs {
    /// Provider ID (e.g. "my-provider").
    #[arg(long)]
    pub id: Option<String>,

    /// Display name (e.g. "My Provider").
    #[arg(long)]
    pub name: Option<String>,

    /// Base URL (e.g. "https://api.example.com/v1").
    #[arg(long)]
    pub base_url: Option<String>,

    /// Wire protocol: "chat_completions" or "responses".
    #[arg(long, default_value = "chat_completions")]
    pub protocol: String,

    /// Environment variable holding the API key (e.g. "MY_PROVIDER_API_KEY").
    #[arg(long)]
    pub env_key: Option<String>,

    /// Bearer token to use directly (not recommended, prefer env_key).
    #[arg(long)]
    pub bearer_token: Option<String>,
}

#[derive(Debug, clap::Parser)]
pub struct RemoveArgs {
    /// Provider ID to remove.
    pub id: String,
}

#[derive(Debug, clap::Parser)]
pub struct TestArgs {
    /// Provider ID to test.
    pub id: String,

    /// Model to use for the test request.
    #[arg(long)]
    pub model: Option<String>,
}

#[derive(Debug, clap::Parser)]
pub struct EditArgs {
    /// Provider ID to edit (must be a custom provider).
    pub id: String,

    /// New display name.
    #[arg(long)]
    pub name: Option<String>,

    /// New base URL.
    #[arg(long)]
    pub base_url: Option<String>,

    /// New wire protocol: "chat_completions" or "responses".
    #[arg(long)]
    pub protocol: Option<String>,

    /// New environment variable for API key.
    #[arg(long)]
    pub env_key: Option<String>,

    /// New bearer token (not recommended, prefer env_key).
    #[arg(long)]
    pub bearer_token: Option<String>,
}

#[derive(Debug, clap::Parser)]
pub struct ModelsArgs {
    /// Provider ID to list models for.
    pub id: String,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Parser)]
pub struct LoginArgs {
    /// Provider ID to authenticate with.
    pub id: String,

    /// API key to set (will prompt if not provided).
    #[arg(long)]
    pub api_key: Option<String>,

    /// Name of the environment variable to write to (defaults to provider's env_key).
    #[arg(long)]
    pub env_key: Option<String>,
}

#[derive(Debug, clap::Parser)]
pub struct StatusArgs {
    /// Only check custom providers (exclude built-in).
    #[arg(long)]
    pub custom_only: bool,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

impl ProviderCli {
    pub async fn run(self, loader_overrides: codex_core::config::LoaderOverrides) -> Result<()> {
        let ProviderCli {
            config_overrides,
            subcommand,
        } = self;

        match subcommand {
            ProviderSubcommand::List(args) => {
                let config = cloud_config::load_config(&config_overrides, loader_overrides).await?;
                run_list(&config, args)?;
            }
            ProviderSubcommand::Add(args) => {
                run_add(&config_overrides, args).await?;
            }
            ProviderSubcommand::Edit(args) => {
                run_edit(&config_overrides, args).await?;
            }
            ProviderSubcommand::Remove(args) => {
                run_remove(&config_overrides, args).await?;
            }
            ProviderSubcommand::Models(args) => {
                let config = cloud_config::load_config(&config_overrides, loader_overrides).await?;
                run_models(&config, args).await?;
            }
            ProviderSubcommand::Test(args) => {
                let config = cloud_config::load_config(&config_overrides, loader_overrides).await?;
                run_test(&config, args).await?;
            }
            ProviderSubcommand::Login(args) => {
                run_login(&config_overrides, args).await?;
            }
            ProviderSubcommand::Status(args) => {
                let config = cloud_config::load_config(&config_overrides, loader_overrides).await?;
                run_status(&config, args).await?;
            }
        }

    Ok(())
}

#[derive(serde::Serialize)]
struct ProviderStatus {
    id: String,
    name: String,
    protocol: String,
    has_api_key: bool,
    status: String,
    status_code: Option<u16>,
    latency_ms: Option<u64>,
    error: Option<String>,
}

async fn run_status(config: &codex_core::config::Config, args: StatusArgs) -> Result<()> {
    let providers = &config.model_providers;

    let providers_to_check: Vec<_> = if args.custom_only {
        providers
            .iter()
            .filter(|(k, _)| !BUILT_IN_PROVIDER_IDS.contains(&k.as_str()))
            .collect()
    } else {
        providers.iter().collect()
    };

    if providers_to_check.is_empty() {
        println!("No providers to check.");
        return Ok(());
    }

    println!("Checking {} providers...\n", providers_to_check.len());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let mut statuses = Vec::new();

    for (id, info) in &providers_to_check {
        let has_api_key = info
            .env_key
            .as_ref()
            .is_some_and(|key| std::env::var(key).is_ok());

        let base_url = info.base_url.as_deref().unwrap_or("https://api.openai.com/v1");
        let url = format!("{base_url}/models");

        let start = std::time::Instant::now();
        let mut request = client.get(&url);

        if let Some(ref env_key) = info.env_key {
            if let Ok(key) = std::env::var(env_key) {
                request = request.bearer_auth(&key);
            }
        }

        if let Some(ref headers) = info.http_headers {
            for (name, value) in headers {
                request = request.header(name, value.as_str());
            }
        }

        let (status_str, status_code, latency_ms, error) = match request.send().await {
            Ok(response) => {
                let latency = start.elapsed().as_millis() as u64;
                let code = response.status().as_u16();
                let status = if response.status().is_success() {
                    "healthy".to_string()
                } else if code == 401 {
                    "auth_failed".to_string()
                } else if code == 403 {
                    "forbidden".to_string()
                } else if code == 429 {
                    "rate_limited".to_string()
                } else {
                    format!("error_{code}")
                };
                (status, Some(code), Some(latency), None)
            }
            Err(e) => {
                let latency = start.elapsed().as_millis() as u64;
                ("unreachable".to_string(), None, Some(latency), Some(e.to_string()))
            }
        };

        statuses.push(ProviderStatus {
            id: id.to_string(),
            name: info.name.clone(),
            protocol: info.wire_api.to_string(),
            has_api_key,
            status: status_str,
            status_code,
            latency_ms,
            error,
        });
    }

    if args.json {
        let output = serde_json::to_string_pretty(&statuses)?;
        println!("{output}");
        return Ok(());
    }

    println!(
        "{:<20} {:<25} {:<15} {:<8} {:<12} {:<10} {}",
        "ID", "Name", "Protocol", "Key", "Status", "Latency", "Code"
    );
    println!("{}", "-".repeat(105));

    for s in &statuses {
        let key = if s.has_api_key { "yes" } else { "no" };
        let latency = s
            .latency_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_default();
        let code = s
            .status_code
            .map(|c| c.to_string())
            .unwrap_or_default();
        let status_display = match s.status.as_str() {
            "healthy" => "healthy".to_string(),
            "auth_failed" => "AUTH FAILED".to_string(),
            "forbidden" => "FORBIDDEN".to_string(),
            "rate_limited" => "RATE LIMITED".to_string(),
            "unreachable" => "UNREACHABLE".to_string(),
            other => other.to_string(),
        };

        println!(
            "{:<20} {:<25} {:<15} {:<8} {:<12} {:<10} {}",
            s.id, s.name, s.protocol, key, status_display, latency, code,
        );
    }

    let healthy = statuses.iter().filter(|s| s.status == "healthy").count();
    let total = statuses.len();
    println!();
    println!("{healthy}/{total} providers healthy.");

    Ok(())
}

fn run_list(config: &codex_core::config::Config, args: ListArgs) -> Result<()> {
    let providers = config.model_providers.clone();

    let built_ins: HashMap<&str, &ModelProviderInfo> = providers
        .iter()
        .filter(|(k, _)| BUILT_IN_PROVIDER_IDS.contains(&k.as_str()))
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    let custom: HashMap<&str, &ModelProviderInfo> = providers
        .iter()
        .filter(|(k, _)| !BUILT_IN_PROVIDER_IDS.contains(&k.as_str()))
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    if args.json {
        let output = if args.custom_only {
            serde_json::to_string_pretty(&custom)?
        } else {
            serde_json::to_string_pretty(&providers)?
        };
        println!("{output}");
        return Ok(());
    }

    if !args.custom_only {
        println!("\nBuilt-in providers:");
        println!("{:<20} {:<30} {:<20}", "ID", "Name", "Protocol");
        println!("{}", "-".repeat(70));
        let mut built_in_list: Vec<_> = built_ins.into_iter().collect();
        built_in_list.sort_by_key(|(k, _)| *k);
        for (id, info) in built_in_list {
            println!(
                "{:<20} {:<30} {:<20}",
                id,
                info.name,
                info.wire_api.to_string(),
            );
        }
    }

    if custom.is_empty() && !args.custom_only {
        println!("\nNo custom providers configured.");
        println!("Use `codex provider add` to add one.");
    } else if !custom.is_empty() {
        if args.custom_only {
            println!("\nCustom providers:");
        } else {
            println!("\nCustom providers:");
        }
        println!("{:<20} {:<30} {:<20} {}", "ID", "Name", "Protocol", "Base URL");
        println!("{}", "-".repeat(90));
        let mut custom_list: Vec<_> = custom.into_iter().collect();
        custom_list.sort_by_key(|(k, _)| *k);
        for (id, info) in custom_list {
            println!(
                "{:<20} {:<30} {:<20} {}",
                id,
                info.name,
                info.wire_api.to_string(),
                info.base_url.as_deref().unwrap_or("(default)"),
            );
        }
    }

    Ok(())
}

async fn run_add(config_overrides: &CliConfigOverrides, args: AddArgs) -> Result<()> {
    let overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let _config = codex_core::config::Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;

    let id = match args.id {
        Some(id) => id,
        None => {
            eprint!("Provider ID: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    if id.is_empty() {
        bail!("Provider ID is required");
    }

    if BUILT_IN_PROVIDER_IDS.contains(&id.as_str()) {
        bail!(
            "Cannot add provider with built-in ID `{id}`. \
             Choose a different ID (e.g. `{id}-custom`)."
        );
    }

    let name = match args.name {
        Some(name) => name,
        None => {
            eprint!("Display name [{id}]: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();
            if input.is_empty() { id.clone() } else { input }
        }
    };

    let base_url = match args.base_url {
        Some(url) => url,
        None => {
            eprint!("Base URL (e.g. https://api.example.com/v1): ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            input.trim().to_string()
        }
    };

    if base_url.is_empty() {
        bail!("Base URL is required");
    }

    let wire_api = match args.protocol.as_str() {
        "chat_completions" | "chat" => WireApi::ChatCompletions,
        "responses" => WireApi::Responses,
        other => bail!("Unknown protocol `{other}`. Use `chat_completions` or `responses`."),
    };

    let env_key = match args.env_key {
        Some(key) => Some(key),
        None => {
            eprint!("Environment variable for API key (e.g. MY_API_KEY) [none]: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();
            if input.is_empty() { None } else { Some(input) }
        }
    };

    let provider = ModelProviderInfo {
        name,
        base_url: Some(base_url),
        env_key,
        env_key_instructions: None,
        experimental_bearer_token: args.bearer_token.map(Into::into),
        auth: None,
        aws: None,
        wire_api,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_openai_auth: false,
        supports_websockets: false,
        supports_standalone_web_search: false,
    };

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let config_path = codex_home.join("config.toml");

    let mut doc: String = if config_path.exists() {
        std::fs::read_to_string(&config_path)?
    } else {
        String::new()
    };

    // Build the TOML entry for this provider
    let mut table = String::new();
    table.push_str(&format!("[model_providers.{id}]\n"));
    table.push_str(&format!("name = {:?}\n", provider.name));
    if let Some(ref url) = provider.base_url {
        table.push_str(&format!("base_url = {:?}\n", url));
    }
    table.push_str(&format!("wire_api = {:?}\n", provider.wire_api.to_string()));
    if let Some(ref key) = provider.env_key {
        table.push_str(&format!("env_key = {:?}\n", key));
    }
    if let Some(ref token) = provider.experimental_bearer_token {
        table.push_str(&format!("experimental_bearer_token = {:?}\n", token.as_str()));
    }

    // Check if [model_providers] section exists
    if doc.contains("[model_providers") {
        // Find the last [model_providers.X] section and append after it
        // Simple approach: append at end of file
        if !doc.ends_with('\n') {
            doc.push('\n');
        }
        doc.push('\n');
        doc.push_str(&table);
    } else if doc.is_empty() || doc.trim().is_empty() {
        doc = format!("[model_providers.{id}]\n");
        doc.push_str(&table[table.find('\n').unwrap() + 1..].trim_start());
        if !doc.ends_with('\n') {
            doc.push('\n');
        }
    } else {
        // Add [model_providers] section at end
        if !doc.ends_with('\n') {
            doc.push('\n');
        }
        doc.push('\n');
        doc.push_str(&table);
    }

    std::fs::write(&config_path, &doc)?;

    println!("Provider `{id}` added successfully.");
    println!("Config: {}", config_path.display());

    Ok(())
}

async fn run_remove(config_overrides: &CliConfigOverrides, args: RemoveArgs) -> Result<()> {
    let overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let _config = codex_core::config::Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;

    let id = &args.id;

    if BUILT_IN_PROVIDER_IDS.contains(&id.as_str()) {
        bail!("Cannot remove built-in provider `{id}`.");
    }

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let config_path = codex_home.join("config.toml");

    if !config_path.exists() {
        bail!("No config file found at {}", config_path.display());
    }

    let doc = std::fs::read_to_string(&config_path)?;

    // Find and remove the [model_providers.ID] section
    let section_header = format!("[model_providers.{id}]");
    if !doc.contains(&section_header) {
        bail!("Provider `{id}` not found in config.");
    }

    // Simple removal: find the section and remove everything until the next [section]
    let lines: Vec<&str> = doc.lines().collect();
    let mut new_lines = Vec::new();
    let mut skip = false;

    for line in lines {
        if line.trim() == section_header {
            skip = true;
            continue;
        }
        if skip {
            // Stop skipping when we hit the next section header
            if line.trim().starts_with('[') && !line.trim().starts_with("[model_providers") {
                skip = false;
                new_lines.push(line);
            }
            // Also stop if we hit another [model_providers.X] (but don't include it yet)
            else if line.trim().starts_with("[model_providers.") {
                skip = false;
                new_lines.push(line);
            }
            // Skip empty lines right after the section too
            else if line.trim().is_empty() {
                continue;
            }
        } else {
            new_lines.push(line);
        }
    }

    let new_doc = new_lines.join("\n");
    std::fs::write(&config_path, new_doc)?;

    println!("Provider `{id}` removed.");
    Ok(())
}

async fn run_edit(config_overrides: &CliConfigOverrides, args: EditArgs) -> Result<()> {
    let overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let _config = codex_core::config::Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;

    let id = &args.id;

    if BUILT_IN_PROVIDER_IDS.contains(&id.as_str()) {
        bail!("Cannot edit built-in provider `{id}`. Only custom providers can be edited.");
    }

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let config_path = codex_home.join("config.toml");

    if !config_path.exists() {
        bail!("No config file found at {}", config_path.display());
    }

    let doc = std::fs::read_to_string(&config_path)?;

    let section_header = format!("[model_providers.{id}]");
    if !doc.contains(&section_header) {
        bail!("Provider `{id}` not found in config.");
    }

    let lines: Vec<&str> = doc.lines().collect();
    let mut new_lines = Vec::new();
    let mut in_section = false;
    let mut section_lines = Vec::new();

    for line in &lines {
        if line.trim() == section_header {
            in_section = true;
            section_lines.push(*line);
            continue;
        }
        if in_section {
            if line.trim().starts_with('[') && !line.trim().starts_with("[model_providers") {
                in_section = false;
                new_lines.extend(section_lines.iter());
                new_lines.push(line);
            } else if line.trim().starts_with("[model_providers.") {
                in_section = false;
                new_lines.extend(section_lines.iter());
                new_lines.push(line);
            } else {
                section_lines.push(*line);
            }
        } else {
            new_lines.push(line);
        }
    }
    if in_section {
        new_lines.extend(section_lines.iter());
    }

    let mut doc = new_lines.join("\n");

    // Build replacement TOML for the section
    let mut table = String::new();
    table.push_str(&format!("[model_providers.{id}]\n"));

    // Parse existing values from the section to preserve defaults
    let mut existing_name = String::new();
    let mut existing_base_url = String::new();
    let mut existing_wire_api = String::new();
    let mut existing_env_key = String::new();
    let mut existing_bearer_token = String::new();

    for line in section_lines.iter() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name = ") {
            existing_name = val.trim_matches('"').to_string();
        } else if let Some(val) = line.strip_prefix("base_url = ") {
            existing_base_url = val.trim_matches('"').to_string();
        } else if let Some(val) = line.strip_prefix("wire_api = ") {
            existing_wire_api = val.trim_matches('"').to_string();
        } else if let Some(val) = line.strip_prefix("env_key = ") {
            existing_env_key = val.trim_matches('"').to_string();
        } else if let Some(val) = line.strip_prefix("experimental_bearer_token = ") {
            existing_bearer_token = val.trim_matches('"').to_string();
        }
    }

    let name = args.name.unwrap_or(existing_name);
    let base_url = args.base_url.unwrap_or(existing_base_url);
    let wire_api = match args.protocol.as_deref() {
        Some("chat_completions" | "chat") => "chat_completions".to_string(),
        Some("responses") => "responses".to_string(),
        None => existing_wire_api,
        other => bail!("Unknown protocol `{other}`. Use `chat_completions` or `responses`."),
    };
    let env_key = args.env_key.or(if existing_env_key.is_empty() {
        None
    } else {
        Some(existing_env_key)
    });
    let bearer_token = args.bearer_token.or(if existing_bearer_token.is_empty() {
        None
    } else {
        Some(existing_bearer_token)
    });

    table.push_str(&format!("name = {name:?}\n"));
    if !base_url.is_empty() {
        table.push_str(&format!("base_url = {base_url:?}\n"));
    }
    table.push_str(&format!("wire_api = {wire_api:?}\n"));
    if let Some(ref key) = env_key {
        table.push_str(&format!("env_key = {key:?}\n"));
    }
    if let Some(ref token) = bearer_token {
        table.push_str(&format!("experimental_bearer_token = {token:?}\n"));
    }

    // Replace the old section with the new one
    let old_section = format!("[model_providers.{id}]");
    if let Some(start) = doc.find(&old_section) {
        // Find the end of the section (next [ section or end of file)
        let after_start = start + old_section.len();
        let rest = &doc[after_start..];
        let end_offset = rest
            .find("\n[")
            .map(|i| after_start + i + 1)
            .unwrap_or(doc.len());
        doc.replace_range(start..end_offset, &table);
    }

    std::fs::write(&config_path, &doc)?;

    println!("Provider `{id}` updated.");
    println!("Config: {}", config_path.display());

    Ok(())
}

async fn run_models(config: &codex_core::config::Config, args: ModelsArgs) -> Result<()> {
    let providers = &config.model_providers;
    let provider = providers
        .get(&args.id)
        .with_context(|| format!("Provider `{}` not found.", args.id))?;

    let base_url = provider.base_url.as_deref().unwrap_or("https://api.openai.com/v1");
    let url = format!("{base_url}/models");

    println!("Fetching models for: {} ({})", provider.name, args.id);
    println!("URL: {url}");
    println!();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let mut request = client.get(&url);

    // Add API key if available
    if let Some(ref env_key) = provider.env_key {
        if let Ok(key) = std::env::var(env_key) {
            request = request.bearer_auth(&key);
        }
    }

    // Add custom headers
    if let Some(ref headers) = provider.http_headers {
        for (name, value) in headers {
            request = request.header(name, value.as_str());
        }
    }

    let response = request.send().await?;
    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("Request failed with status {status}: {body}");
    }

    let body = response.text().await?;

    // Try to parse as OpenAI-compatible model list
    let models_response: serde_json::Value = serde_json::from_str(&body)?;

    if args.json {
        println!("{body}");
        return Ok(());
    }

    // Extract models array
    let models = models_response
        .get("data")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if models.is_empty() {
        println!("No models found.");
        return Ok(());
    }

    println!("{:<40} {:<20} {:<15}", "Model ID", "Owned By", "Created");
    println!("{}", "-".repeat(75));

    let mut model_list: Vec<_> = models
        .iter()
        .filter_map(|m| {
            let id = m.get("id")?.as_str()?;
            let owned_by = m
                .get("owned_by")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let created = m
                .get("created")
                .and_then(|v| v.as_i64())
                .map(|ts| {
                    let dt = chrono::DateTime::from_timestamp(ts, 0)?;
                    Some(dt.format("%Y-%m-%d").to_string())
                })
                .flatten()
                .unwrap_or_else(|| "unknown".to_string());
            Some((id, owned_by, created))
        })
        .collect();

    model_list.sort_by_key(|(id, _, _)| id.to_string());

    for (id, owned_by, created) in &model_list {
        println!("{id:<40} {owned_by:<20} {created:<15}");
    }

    println!();
    println!("{} models found.", model_list.len());

    Ok(())
}

async fn run_login(config_overrides: &CliConfigOverrides, args: LoginArgs) -> Result<()> {
    let overrides = config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let _config = codex_core::config::Config::load_with_cli_overrides(overrides)
        .await
        .context("failed to load configuration")?;

    let id = &args.id;

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let config_path = codex_home.join("config.toml");

    // Find the provider's env_key from config or built-in
    let env_key = if let Some(key) = args.env_key {
        key
    } else {
        // Try to find the env_key from the provider config
        let doc = if config_path.exists() {
            std::fs::read_to_string(&config_path)?
        } else {
            String::new()
        };

        let section_header = format!("[model_providers.{id}]");
        let mut found_env_key = String::new();

        if doc.contains(&section_header) {
            for line in doc.lines() {
                let line = line.trim();
                if line.starts_with(&section_header) {
                    continue;
                }
                if let Some(val) = line.strip_prefix("env_key = ") {
                    found_env_key = val.trim_matches('"').to_string();
                    break;
                }
            }
        }

        if found_env_key.is_empty() {
            bail!(
                "No env_key found for provider `{id}`. \
                 Use --env-key to specify the environment variable name, \
                 or add env_key to the provider config."
            );
        }
        found_env_key
    };

    // Get the API key
    let api_key = match args.api_key {
        Some(key) => key,
        None => {
            eprint!("Enter API key for {id}: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim().to_string();
            if input.is_empty() {
                bail!("API key is required.");
            }
            input
        }
    };

    // Set the environment variable for the current session
    std::env::set_var(&env_key, &api_key);

    println!("API key set for provider `{id}`.");
    println!("Environment variable: {env_key}");

    // Check if there's already a shell profile entry
    let shell_profile = detect_shell_profile();
    if let Some(profile) = shell_profile {
        let export_line = format!("export {env_key}=\"{api_key}\"");
        let profile_content = std::fs::read_to_string(&profile).unwrap_or_default();

        if profile_content.contains(&format!("export {env_key}=")) {
            println!("\nNote: {env_key} is already set in {}", profile.display());
            println!("To update, edit the file directly or remove the old entry.");
        } else {
            println!("\nTo persist this key, add to your shell profile ({}):", profile.display());
            println!("  {export_line}");
        }
    } else {
        println!("\nTo persist this key, set {env_key} in your environment.");
    }

    Ok(())
}

fn detect_shell_profile() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let shell = std::env::var("SHELL").unwrap_or_default();

    let profile_name = if shell.contains("zsh") {
        ".zshrc"
    } else if shell.contains("bash") {
        ".bashrc"
    } else if shell.contains("fish") {
        Some(".config/fish/config.fish")
    } else {
        ".profile"
    };

    let path = if shell.contains("fish") {
        std::path::PathBuf::from(home).join(profile_name)
    } else {
        std::path::PathBuf::from(home).join(profile_name)
    };

    Some(path)
}

async fn run_test(config: &codex_core::config::Config, args: TestArgs) -> Result<()> {
    let providers = &config.model_providers;
    let provider = providers
        .get(&args.id)
        .with_context(|| format!("Provider `{}` not found.", args.id))?;

    println!("Testing provider: {} ({})", provider.name, args.id);
    println!("Base URL: {}", provider.base_url.as_deref().unwrap_or("(default)"));
    println!("Protocol: {}", provider.wire_api);
    println!();

    // Check if API key is available
    if let Some(ref env_key) = provider.env_key {
        match std::env::var(env_key) {
            Ok(val) if !val.trim().is_empty() => {
                println!("API key: {} = ****{}", env_key, &val[val.len().saturating_sub(4)..]);
            }
            _ => {
                println!("API key: {} is not set or empty", env_key);
                if let Some(ref instructions) = provider.env_key_instructions {
                    println!("  {instructions}");
                }
                bail!("API key environment variable `{env_key}` is not set.");
            }
        }
    } else {
        println!("No API key required (no env_key configured).");
    }

    // Try a lightweight connectivity check
    let base_url = provider.base_url.as_deref().unwrap_or("https://api.openai.com/v1");
    let url = format!("{base_url}/models");

    println!("\nAttempting GET {url} ...");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let mut request = client.get(&url);

    // Add API key if available
    if let Some(ref env_key) = provider.env_key {
        if let Ok(key) = std::env::var(env_key) {
            request = request.bearer_auth(&key);
        }
    }

    // Add custom headers
    if let Some(ref headers) = provider.http_headers {
        for (name, value) in headers {
            request = request.header(name, value.as_str());
        }
    }

    match request.send().await {
        Ok(response) => {
            let status = response.status();
            println!("Status: {status}");

            if status.is_success() {
                println!("Provider is reachable.");
            } else if status.as_u16() == 401 {
                println!("Authentication failed. Check your API key.");
            } else if status.as_u16() == 403 {
                println!("Access denied. Your API key may not have permission.");
            } else if status.as_u16() == 429 {
                println!("Rate limited. The provider is reachable but throttling requests.");
            } else {
                println!("Unexpected status code. The provider may be misconfigured.");
            }
        }
        Err(e) => {
            bail!("Connection failed: {e}");
        }
    }

    Ok(())
}
