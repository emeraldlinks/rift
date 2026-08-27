use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_core::config::find_codex_home;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_models_manager::bundled_models_response;
use codex_protocol::openai_models::ModelPreset;
use codex_utils_cli::CliConfigOverrides;

use crate::cloud_config;

/// Subcommands:
/// - `list` — list available models from the bundled catalog
/// - `search` — search models by name, slug, or description
/// - `use` — set the default model in config.toml
#[derive(Debug, clap::Parser)]
pub struct ModelCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    pub subcommand: ModelSubcommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum ModelSubcommand {
    List(ListModelsArgs),
    Search(SearchModelsArgs),
    Use(UseModelArgs),
}

#[derive(Debug, clap::Parser)]
pub struct ListModelsArgs {
    /// Filter by provider ID (e.g. "openai", "anthropic").
    #[arg(long)]
    pub provider: Option<String>,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,

    /// Show all models including those hidden from the picker.
    #[arg(long)]
    pub all: bool,
}

#[derive(Debug, clap::Parser)]
pub struct SearchModelsArgs {
    /// Search query (matches against model slug, display name, and description).
    pub query: String,

    /// Output as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, clap::Parser)]
pub struct UseModelArgs {
    /// Model slug to set as default (e.g. "gpt-5.4", "o3").
    pub model: String,

    /// Reasoning effort level: low, medium, high.
    #[arg(long)]
    pub effort: Option<String>,
}

impl ModelCli {
    pub async fn run(self, loader_overrides: codex_core::config::LoaderOverrides) -> Result<()> {
        let ModelCli {
            config_overrides,
            subcommand,
        } = self;

        match subcommand {
            ModelSubcommand::List(args) => {
                let config =
                    cloud_config::load_config(&config_overrides, loader_overrides).await?;
                run_list(&config, args)?;
            }
            ModelSubcommand::Search(args) => {
                let config =
                    cloud_config::load_config(&config_overrides, loader_overrides).await?;
                run_search(&config, args)?;
            }
            ModelSubcommand::Use(args) => {
                run_use(&config_overrides, args).await?;
            }
        }

        Ok(())
    }
}

fn get_model_presets(config: &codex_core::config::Config) -> Result<Vec<ModelPreset>> {
    let catalog = match &config.model_catalog {
        Some(catalog) => catalog.clone(),
        None => bundled_models_response()
            .context("failed to load bundled model catalog")?,
    };

    let presets: Vec<ModelPreset> = catalog.models.into_iter().map(Into::into).collect();

    Ok(presets)
}

fn run_list(config: &codex_core::config::Config, args: ListModelsArgs) -> Result<()> {
    let presets = get_model_presets(config)?;

    let presets: Vec<ModelPreset> = if args.all {
        presets
    } else {
        presets.into_iter().filter(|p| p.show_in_picker).collect()
    };

    // Filter by provider if specified
    let presets = if let Some(ref provider_id) = args.provider {
        presets
            .into_iter()
            .filter(|p| p.model.starts_with(provider_id))
            .collect()
    } else {
        presets
    };

    if args.json {
        let output = serde_json::to_string_pretty(&presets)?;
        println!("{output}");
        return Ok(());
    }

    if presets.is_empty() {
        println!("No models available.");
        return Ok(());
    }

    let current_model = config.model.as_deref().unwrap_or("(provider default)");

    println!("Current model: {current_model}");
    println!();
    println!(
        "{:<30} {:<25} {:<15}",
        "Slug", "Display Name", "Reasoning"
    );
    println!("{}", "-".repeat(70));

    let mut sorted = presets;
    sorted.sort_by(|a, b| a.display_name.cmp(&b.display_name));

    for preset in &sorted {
        let default_marker = if preset.is_default { " *" } else { "" };
        println!(
            "{:<30} {:<25} {:<15}{default_marker}",
            preset.model,
            preset.display_name,
            preset.default_reasoning_effort,
        );
    }

    println!();
    println!("{} models available. (* = provider default)", sorted.len());

    Ok(())
}

fn run_search(config: &codex_core::config::Config, args: SearchModelsArgs) -> Result<()> {
    let presets = get_model_presets(config)?;
    let query = args.query.to_lowercase();

    let matches: Vec<&ModelPreset> = presets
        .iter()
        .filter(|p| {
            p.model.to_lowercase().contains(&query)
                || p.display_name.to_lowercase().contains(&query)
                || p.description.to_lowercase().contains(&query)
        })
        .collect();

    if args.json {
        let output = serde_json::to_string_pretty(&matches)?;
        println!("{output}");
        return Ok(());
    }

    if matches.is_empty() {
        println!("No models matching `{}`.", args.query);
        return Ok(());
    }

    println!(
        "{:<30} {:<25} {}",
        "Slug", "Display Name", "Description"
    );
    println!("{}", "-".repeat(100));

    for preset in &matches {
        let desc = if preset.description.chars().count() > 40 {
            let truncated: String = preset.description.chars().take(37).collect();
            format!("{truncated}...")
        } else {
            preset.description.clone()
        };
        println!(
            "{:<30} {:<25} {}",
            preset.model, preset.display_name, desc,
        );
    }

    println!();
    println!("{} models found.", matches.len());

    Ok(())
}

async fn run_use(config_overrides: &CliConfigOverrides, args: UseModelArgs) -> Result<()> {
    let model = &args.model;

    let effort = match args.effort.as_deref() {
        Some("low") => Some(codex_protocol::openai_models::ReasoningEffort::Low),
        Some("medium") => Some(codex_protocol::openai_models::ReasoningEffort::Medium),
        Some("high") => Some(codex_protocol::openai_models::ReasoningEffort::High),
        Some(other) => bail!(
            "Unknown reasoning effort `{other}`. Use `low`, `medium`, or `high`."
        ),
        None => None,
    };

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;

    ConfigEditsBuilder::new(&codex_home)
        .set_model(Some(model), effort)
        .apply()
        .await?;

    println!("Default model set to `{model}`.");

    // Verify by loading config
    let config = cloud_config::load_config(config_overrides, codex_core::config::LoaderOverrides::default()).await
        .context("failed to load configuration")?;

    println!(
        "Active model: {}",
        config.model.as_deref().unwrap_or("(provider default)")
    );
    println!("Provider: {} ({})", config.model_provider.name, config.model_provider_id);

    Ok(())
}
