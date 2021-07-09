use std::env::args;
use std::io::{self, stdin, stdout, Write};

use log::LevelFilter::Info;
use rpassword::read_password_from_tty;
use simple_logger::SimpleLogger;

use crate::github::{GitHub, Image, Version};

mod github;

#[tokio::main]
async fn main() -> reqwest::Result<()> {
    SimpleLogger::new().with_level(Info).init().unwrap();

    let args: Vec<String> = args().collect();

    if args.len() != 3 {
        println!("{} <account> <[org/]image>", args[0]);
        return Ok(());
    }

    let username = &args[1];
    let image = &args[2];

    let token = read_password_from_tty(Some("Please type in your token: ")).unwrap();

    let github = GitHub::new(username.to_string(), token.to_string())?;
    let image = Image::new(image.to_string()).expect("Wrong image format!");

    let versions = github.fetch_versions(&image).await?;

    if versions.is_empty() {
        eprintln!("No versions found.");
        return Ok(());
    }

    let untagged_versions: Vec<&Version> = versions
        .iter()
        .filter(|version| version.metadata.container.tags.is_empty())
        .collect();

    println!();

    if untagged_versions.is_empty() {
        eprintln!("No untagged versions found.");
        return Ok(());
    }

    println!("Found {} untagged versions:", untagged_versions.len());

    for version in &untagged_versions {
        println!("- {}: {}", version.name, version.html_url);
    }

    println!();
    let yes_no = ask("Do you want to delete the displayed versions? [Y/n]: ").unwrap();
    println!();

    if yes_no.is_empty() || !yes_no.eq_ignore_ascii_case("y") {
        println!("Nothing will be deleted.");
        return Ok(());
    }

    println!("Deleting {} untagged versions...", untagged_versions.len());

    github.delete_versions(&image, &untagged_versions).await?;

    println!("Success!");

    Ok(())
}

fn ask(question: &str) -> io::Result<String> {
    print!("{}", question);
    stdout().flush().unwrap();

    let mut value = String::new();
    stdin().read_line(&mut value)?;

    Ok(value.trim().to_owned())
}
