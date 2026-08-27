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
    Remove(RemoveArgs),
    Test(TestArgs),
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
            ProviderSubcommand::Remove(args) => {
                run_remove(&config_overrides, args).await?;
            }
            ProviderSubcommand::Test(args) => {
                let config = cloud_config::load_config(&config_overrides, loader_overrides).await?;
                run_test(&config, args).await?;
            }
        }

        Ok(())
    }
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
