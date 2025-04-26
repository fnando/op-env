use std::process::{exit, Command};

use clap::Parser;
use serde::Deserialize;
use serde_json::{to_string_pretty, Map, Value};

/// A command-line tool to retrieve secrets from 1Password, with custom output
/// to several formats. Depends on 1Password's own `op` command.
#[derive(Parser, Debug)]
#[command(version)]
struct Cmd {
    /// The output format.
    #[arg(short, long, default_value = "dotenv", value_parser = clap::builder::PossibleValuesParser::new(["json", "dotenv", "shell"]))]
    format: String,

    /// The 1password vault that contains the secrets.
    #[arg(short, long, default_value = "dev")]
    vault: String,

    /// The 1password item that contains the secrets.
    #[arg(short, long, default_value = "main")]
    item: String,
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error(transparent)]
    OpFailed(#[from] std::io::Error),

    #[error(transparent)]
    InvalidJson(#[from] serde_json::Error),
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
            eprintln!("{e}");
            exit(1);
        }
    };
}

fn run() -> Result<(), Error> {
    let cmd = Cmd::parse();
    let result = Command::new("op")
        .arg("item")
        .arg("get")
        .arg(cmd.item)
        .arg("--vault")
        .arg(cmd.vault)
        .arg("--format")
        .arg("json")
        .output()?;

    let json = String::from_utf8_lossy(&result.stdout);
    let vault: Vault = serde_json::from_str(&json)?;
    let fields: Vec<&VaultField> = vault.fields.iter().filter(|i| i.value.is_some()).collect();

    if cmd.format == "json" {
        output_json(fields)?;
    } else if cmd.format == "dotenv" {
        output_dotenv(fields);
    } else if cmd.format == "shell" {
        output_shell(fields);
    }

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
