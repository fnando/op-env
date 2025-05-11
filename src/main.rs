use dotenvy::from_path_iter;
use std::collections::HashMap;
use std::io::{self};
use std::path::PathBuf;
use std::process::{exit, Command};

use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::{to_string_pretty, Map, Value};

/// A command-line tool to retrieve secrets from 1Password, with custom output
/// to several formats. Depends on 1Password's own `op` command.
#[derive(Parser, Debug)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// The 1password vault that contains the secrets.
    #[arg(short, long, default_value = "dev")]
    vault: String,

    /// The 1password item that contains the secrets.
    #[arg(short, long, default_value = "main")]
    item: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Export secrets from 1Password
    Export {
        /// The output format.
        #[arg(short, long, default_value = "dotenv", value_parser = clap::builder::PossibleValuesParser::new(["json", "dotenv", "shell"]))]
        format: String,
    },

    /// Import secrets from a file into 1Password
    Import {
        /// Path to the file containing secrets
        #[arg(short, long, required = true)]
        env_file: PathBuf,
    },
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error(transparent)]
    OpFailed(#[from] std::io::Error),

    #[error(transparent)]
    InvalidJson(#[from] serde_json::Error),

    #[error("Unable to parse dotenv file: {0}")]
    DotenvParse(String),
}

#[derive(Deserialize, Debug)]
struct Vault {
    fields: Vec<VaultField>,
}

#[derive(Deserialize, Debug)]
struct VaultField {
    value: Option<String>,
    label: String,
}

fn main() {
    match run() {
        Ok(()) => (),
        Err(e) => {
            eprintln!("ERROR: {e}");
            exit(1);
        }
    };
}

fn run() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Import { env_file }) => import_secrets(env_file, &cli.vault, &cli.item),
        Some(Commands::Export { format }) => export_secrets(format, &cli.vault, &cli.item),
        None => {
            println!("No command provided. Use --help for more information.");
            Ok(())
        }
    }
}

fn export_secrets(format: &str, vault: &str, item: &str) -> Result<(), Error> {
    let result = Command::new("op")
        .arg("item")
        .arg("get")
        .arg(item)
        .arg("--vault")
        .arg(vault)
        .arg("--format")
        .arg("json")
        .output()?;

    let json = String::from_utf8_lossy(&result.stdout);
    let vault: Vault = serde_json::from_str(&json)?;
    let fields: Vec<&VaultField> = vault.fields.iter().filter(|i| i.value.is_some()).collect();

    if format == "json" {
        output_json(fields)?;
    } else if format == "dotenv" {
        output_dotenv(fields);
    } else if format == "shell" {
        output_shell(fields);
    }

    Ok(())
}

fn import_secrets(file: &PathBuf, vault: &str, item: &str) -> Result<(), Error> {
    let env_vars: HashMap<String, String> = from_path_iter(file)
        .map_err(|e| Error::DotenvParse(e.to_string()))?
        .collect::<Result<HashMap<_, _>, _>>()
        .map_err(|e| Error::DotenvParse(e.to_string()))?;

    for (key, value) in env_vars {
        let output = Command::new("op")
            .arg("item")
            .arg("edit")
            .arg(item)
            .arg("--vault")
            .arg(vault)
            .arg(format!("{key}={value}"))
            .output()?;

        if !output.status.success() {
            return Err(Error::OpFailed(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to import secret: {key}"),
            )));
        }
    }

    println!("All secrets imported successfully!");
    Ok(())
}

fn output_json(fields: Vec<&VaultField>) -> Result<(), Error> {
    let mut map = Map::new();

    for field in fields {
        let value: String = field.value.clone().unwrap();
        map.insert(field.label.clone(), Value::String(value));
    }

    let json_output = Value::Object(map);
    println!("{}", to_string_pretty(&json_output)?);

    Ok(())
}

fn output_dotenv(mut fields: Vec<&VaultField>) {
    fields.sort_by_key(|field| field.label.to_lowercase());

    for field in fields {
        let value: String = field.value.clone().unwrap();
        println!(
            "{}={}",
            shell_escape::escape(field.label.clone().into()),
            shell_escape::escape(value.clone().into()),
        );
    }
}

fn output_shell(mut fields: Vec<&VaultField>) {
    fields.sort_by_key(|field| field.label.to_lowercase());

    for field in fields {
        let value: String = field.value.clone().unwrap();
        println!(
            "export {}={}",
            shell_escape::escape(field.label.clone().into()),
            shell_escape::escape(value.clone().into()),
        );
    }
}
