//! A headless command line interface for scripted use.
//!
//! The terminal UI is built for people; this module is built for scripts. It
//! exposes the same provider engine through a small, stable command surface:
//!
//! ```text
//! moviebox-cli providers
//! moviebox-cli search "dune" --json
//! moviebox-cli details <subject-id> --json
//! moviebox-cli streams <subject-id> --season 1 --episode 2 --json
//! moviebox-cli play <subject-id> --season 1 --episode 2
//! ```
//!
//! With `--json` the output is a single JSON document on stdout, so it can be
//! piped into `jq` without any parsing.

use serde_json::{Value, json};

use crate::providers::models::{ProviderKind, Release};
use crate::service::MovieBoxService;

/// The usage text, shown when the arguments do not parse.
pub const USAGE: &str = "\
MovieBox headless CLI

USAGE:
    moviebox-cli <COMMAND> [OPTIONS]

COMMANDS:
    providers                 List the providers and their capabilities
    search <query>            Search the catalog
    details <subject-id>      Metadata, seasons and episodes of a title
    streams <subject-id>      The playable sources of a title or episode
    play <subject-id>         Resolve a source and hand it to a player

OPTIONS:
    --provider <id>           Provider to use (default: moviebox)
    --page <n>                Search page (default: 1)
    --limit <n>               Maximum number of results
    --season <n>              Season of the episode
    --episode <n>             Episode of the season
    --player <kind>           mpv, vlc, iina or android
    --print-url               Print the resolved url and headers instead of playing
    --json                    Machine readable output
    -h, --help                Show this help";

/// The command the user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Providers,
    Search { query: String },
    Details { subject_id: String },
    Streams { subject_id: String },
    Play { subject_id: String },
}

/// The parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub command: Command,
    pub provider: Option<String>,
    pub page: usize,
    pub limit: Option<usize>,
    pub season: usize,
    pub episode: usize,
    pub player: Option<String>,
    pub json: bool,
    pub print_url: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            command: Command::Providers,
            provider: None,
            page: 1,
            limit: None,
            season: 0,
            episode: 0,
            player: None,
            json: false,
            print_url: false,
        }
    }
}

/// Parses the arguments (without the program name).
pub fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut positional: Vec<String> = Vec::new();
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--provider" => {
                index += 1;
                options.provider = Some(value(args, index, "--provider")?);
            }
            "--page" => {
                index += 1;
                options.page = parse_number(value(args, index, "--page")?, "--page")?;
            }
            "--limit" => {
                index += 1;
                options.limit = Some(parse_number(value(args, index, "--limit")?, "--limit")?);
            }
            "--season" => {
                index += 1;
                options.season = parse_number(value(args, index, "--season")?, "--season")?;
            }
            "--episode" => {
                index += 1;
                options.episode = parse_number(value(args, index, "--episode")?, "--episode")?;
            }
            "--player" => {
                index += 1;
                options.player = Some(value(args, index, "--player")?);
            }
            "--json" => options.json = true,
            "--print-url" => options.print_url = true,
            "-h" | "--help" => return Err(USAGE.to_string()),
            other if other.starts_with('-') => {
                return Err(format!("unknown option: {other}"));
            }
            other => positional.push(other.to_string()),
        }
        index += 1;
    }

    let command = match positional.first().map(String::as_str) {
        Some("providers") | None => Command::Providers,
        Some("search") => {
            let query = positional
                .get(1)
                .cloned()
                .ok_or_else(|| "search needs a query".to_string())?;
            Command::Search { query }
        }
        Some("details") => {
            let subject_id = positional
                .get(1)
                .cloned()
                .ok_or_else(|| "details needs a subject id".to_string())?;
            Command::Details { subject_id }
        }
        Some("streams") => {
            let subject_id = positional
                .get(1)
                .cloned()
                .ok_or_else(|| "streams needs a subject id".to_string())?;
            Command::Streams { subject_id }
        }
        Some("play") => {
            let subject_id = positional
                .get(1)
                .cloned()
                .ok_or_else(|| "play needs a subject id".to_string())?;
            Command::Play { subject_id }
        }
        Some(other) => return Err(format!("unknown command: {other}")),
    };

    options.command = command;
    Ok(options)
}

fn value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index)
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

fn parse_number(raw: String, flag: &str) -> Result<usize, String> {
    raw.parse::<usize>()
        .map_err(|_| format!("{flag} needs a number, got: {raw}"))
}

/// Runs the command and returns the text to print.
pub async fn run(options: &Options) -> Result<String, String> {
    let service = MovieBoxService::new();
    let provider = match options.provider.as_deref() {
        Some(name) => {
            ProviderKind::parse(name).ok_or_else(|| format!("unknown provider: {name}"))?
        }
        None => ProviderKind::MovieBox,
    };

    match &options.command {
        Command::Providers => {
            let providers: Vec<Value> = ProviderKind::ENABLED
                .iter()
                .map(|kind| {
                    let capabilities = service.capabilities(*kind);
                    json!({
                        "id": kind.cache_key(),
                        "label": kind.label(),
                        "supportsSearch": capabilities.supports_search,
                        "supportsPagination": capabilities.supports_pagination,
                        "supportsSeries": capabilities.supports_series,
                        "supportsSubtitles": capabilities.supports_subtitles,
                        "supportsHomepage": capabilities.supports_homepage,
                    })
                })
                .collect();
            if options.json {
                Ok(json!({ "providers": providers }).to_string())
            } else {
                let mut lines = Vec::new();
                for provider in providers {
                    lines.push(format!(
                        "{:<16} {}",
                        provider["label"].as_str().unwrap_or_default(),
                        provider["id"].as_str().unwrap_or_default()
                    ));
                }
                Ok(lines.join("\n"))
            }
        }
        Command::Search { query } => {
            let items = service
                .search_typed(provider, query, options.page)
                .await
                .map_err(|error| error.to_string())?;
            let items: Vec<&crate::providers::models::CatalogItem> = match options.limit {
                Some(limit) => items.iter().take(limit).collect(),
                None => items.iter().collect(),
            };
            if options.json {
                Ok(json!({ "items": items }).to_string())
            } else {
                let mut lines = Vec::new();
                for item in items {
                    let mut line = format!(
                        "{:<20} {:<28} {}",
                        item.id.value,
                        item.title,
                        item.year.clone().unwrap_or_default()
                    );
                    line.push_str(&format!("  [{}]", item.id.provider.label()));
                    lines.push(line);
                }
                Ok(lines.join("\n"))
            }
        }
        Command::Details { subject_id } => {
            let details = service
                .details_typed(provider, subject_id)
                .await
                .map_err(|error| error.to_string())?;
            if options.json {
                Ok(json!({ "details": details }).to_string())
            } else {
                let mut lines = vec![
                    details.title.clone(),
                    details.year.clone().unwrap_or_else(|| "-".to_string()),
                    details
                        .description
                        .clone()
                        .unwrap_or_else(|| "-".to_string()),
                ];
                for season in &details.seasons {
                    lines.push(format!(
                        "season {} ({} episodes)",
                        season.number,
                        season.episodes.len()
                    ));
                }
                Ok(lines.join("\n"))
            }
        }
        Command::Streams { subject_id } => {
            let releases = service
                .episode_streams(provider, subject_id, options.season, options.episode)
                .await
                .map_err(|error| error.to_string())?;
            if options.json {
                Ok(json!({ "releases": releases }).to_string())
            } else {
                Ok(format_releases(&releases))
            }
        }
        Command::Play { subject_id } => {
            let releases = service
                .episode_streams(provider, subject_id, options.season, options.episode)
                .await
                .map_err(|error| error.to_string())?;
            let release = releases
                .first()
                .ok_or_else(|| "no playable source was found".to_string())?;
            let source = service
                .resolve_playback(provider, subject_id, release)
                .await
                .ok_or_else(|| "the source could not be resolved".to_string())?;

            if options.print_url {
                if options.json {
                    return Ok(json!({
                        "url": source.url,
                        "headers": source.headers,
                        "sourceLabel": source.source_label,
                    })
                    .to_string());
                }
                let mut lines = vec![source.url.clone()];
                for (name, value) in &source.headers {
                    lines.push(format!("{name}: {value}"));
                }
                return Ok(lines.join("\n"));
            }

            play(&service, &source, release, options).await
        }
    }
}

/// Hands the resolved source to an external player.
async fn play(
    service: &MovieBoxService,
    source: &crate::providers::models::PlaybackSource,
    release: &Release,
    options: &Options,
) -> Result<String, String> {
    use crate::player::{self, PlayerKind};

    let kind = match options.player.as_deref() {
        Some(name) => PlayerKind::parse(name).ok_or_else(|| format!("unknown player: {name}"))?,
        None => player::detect()
            .into_iter()
            .find(|kind| player::supports_headers(*kind, &source.headers))
            .or_else(|| player::detect().into_iter().next())
            .ok_or_else(|| "no supported player was found".to_string())?,
    };

    if !source.headers.is_empty() && !player::supports_headers(kind, &source.headers) {
        return Err(format!(
            "{} cannot play a source that needs request headers, use --player mpv",
            kind.label()
        ));
    }

    let max_height = Some(release.resolution_u64()).filter(|height| *height > 0);
    let mut command = player::command(
        kind,
        &source.url,
        None,
        &source.headers,
        None,
        None,
        None,
        max_height,
    );
    let status = command
        .status()
        .map_err(|error| format!("could not start {}: {error}", kind.label()))?;
    if !status.success() {
        return Err(format!(
            "{} exited with {}",
            kind.label(),
            status.code().unwrap_or(-1)
        ));
    }
    let _ = service;
    Ok(format!("played {} with {}", release.filename, kind.label()))
}

/// The human readable source list.
fn format_releases(releases: &[Release]) -> String {
    let mut lines = Vec::new();
    for release in releases {
        let url = release
            .mirrors
            .first()
            .map(|mirror| mirror.resolver_url.as_str())
            .unwrap_or("-");
        lines.push(format!(
            "{:<52} {:<8} {:<6} {}",
            release.filename,
            release.quality.clone().unwrap_or_else(|| "-".to_string()),
            release.codec.clone().unwrap_or_else(|| "-".to_string()),
            url
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_the_search_command() {
        let options = parse_args(&args(&["search", "dune", "--json"])).expect("options");
        assert_eq!(
            options.command,
            Command::Search {
                query: "dune".to_string()
            }
        );
        assert!(options.json);
        assert_eq!(options.page, 1);
    }

    #[test]
    fn parses_the_options_of_the_streams_command() {
        let options = parse_args(&args(&[
            "streams",
            "12345",
            "--provider",
            "moviebox",
            "--season",
            "1",
            "--episode",
            "4",
        ]))
        .expect("options");
        assert_eq!(
            options.command,
            Command::Streams {
                subject_id: "12345".to_string()
            }
        );
        assert_eq!(options.provider.as_deref(), Some("moviebox"));
        assert_eq!(options.season, 1);
        assert_eq!(options.episode, 4);
    }

    #[test]
    fn rejects_unknown_commands_and_options() {
        assert!(parse_args(&args(&["nope"])).is_err());
        assert!(parse_args(&args(&["search", "dune", "--nope"])).is_err());
        assert!(parse_args(&args(&["search", "dune", "--page"])).is_err());
        assert!(parse_args(&args(&["search", "dune", "--page", "x"])).is_err());
    }

    #[test]
    fn defaults_to_the_providers_command() {
        let options = parse_args(&args(&[])).expect("options");
        assert_eq!(options.command, Command::Providers);
    }

    #[test]
    fn formats_the_release_list() {
        let releases = vec![Release {
            provider: ProviderKind::MovieBox,
            filename: "Dune [Hindi]-1080P".to_string(),
            quality: Some("1080p".to_string()),
            codec: Some("h264".to_string()),
            language: None,
            size_bytes: Some(1_823_553_113),
            season: Some(0),
            episode: Some(0),
            mirrors: vec![crate::providers::models::SourceMirror {
                label: "adilessa".to_string(),
                resolver_url: "https://cdn.test/a.mp4".to_string(),
                headers: vec![],
                direct_file: true,
            }],
            resource_id: Some("1".to_string()),
        }];
        let text = format_releases(&releases);
        assert!(text.contains("Dune [Hindi]-1080P"));
        assert!(text.contains("1080p"));
        assert!(text.contains("https://cdn.test/a.mp4"));
    }
}
