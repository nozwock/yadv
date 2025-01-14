use anyhow::{bail, Context, Result};
use chrono::Datelike;
use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::{fmt, fs, io::Write, process, time::Duration};
use yaadv::{api::fetch_inputs, args::Cli, config::Config, credentials::Secrets, inputs::AdvInput};

fn download_inputs(inputs: &Vec<AdvInput>, session_token: &str) -> Result<Vec<String>> {
    fs::create_dir_all(
        inputs
            .iter()
            .next()
            .context("no input file to download")?
            .path()
            .parent()
            .context("no parent folder exists")?,
    )?;

    let mut out_err = vec![];

    for (input, resp) in fetch_inputs(inputs, session_token)
        .into_iter()
        .enumerate()
        .map(|(i, resp)| (&inputs[i], resp))
    {
        match resp {
            Ok(resp) => {
                fs::File::create(input.path())?.write_all(resp.into_string()?.as_bytes())?
            }
            Err(err) => {
                if let ureq::Error::Status(err_code, _) = err {
                    if err_code == 404 {
                        out_err.push(format!(
                            "{} {} {} {}",
                            "Error 404:".red(),
                            "Day".red(),
                            input.day.to_string().red(),
                            "is either not unlocked yet or doesn't exist".red()
                        ));
                    } else {
                        // for any error other than 404; just abort
                        bail!(
                            "unhandled error while downloading input files!\n{}",
                            err.to_string()
                        )
                    }
                }
            }
        };
    }

    Ok(out_err)
}

#[derive(Debug)]
enum CredentialsOption {
    ViewToken,
    SetToken,
}

impl fmt::Display for CredentialsOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CredentialsOption::ViewToken => write!(f, "View stored token"),
            CredentialsOption::SetToken => write!(f, "Set a new token"),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        yaadv::args::Commands::Inputs(inputs) => {
            let cfg = Config::load();
            if inputs.config_exists {
                if cfg.is_none() {
                    eprintln!("{}", "Could not find the config file in pwd".red());
                    process::exit(2);
                }
            }
            let cfg = cfg.unwrap_or_default();

            let days = if let Some(day) = inputs.day {
                vec![day]
            } else {
                (1..=25).collect()
            };

            let year = if let Some(year) = inputs.year {
                year
            } else {
                let curr = chrono::Utc::now().naive_utc();
                let mut yr = curr.year();
                // since AOC starts in december
                if curr.month() != 12 {
                    yr -= 1;
                }
                yr
            };

            let sp = ProgressBar::new_spinner();
            sp.set_message("Downloading...");
            sp.enable_steady_tick(Duration::from_millis(80));
            sp.set_style(
                ProgressStyle::with_template("{spinner:.blue} {msg}")
                    .unwrap()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
            );

            let inputs = days
                .into_iter()
                .map(|day| {
                    AdvInput::new(day, year).with_formatted_path(
                        if inputs.formatted_path.is_some() {
                            inputs.formatted_path.as_ref().map(|s| s.as_str())
                        } else {
                            // try to use path from cfg located in pwd
                            cfg.path.as_ref().map(|s| s.as_str())
                        },
                    )
                })
                .collect();

            let errs = download_inputs(
                &inputs,
                &Secrets::load()
                    .session_token
                    .context("No session token found!\nPlease add a sesssion token first")?,
            )?;

            sp.finish_and_clear();
            errs.into_iter().for_each(|err| eprintln!("{}", err));
            eprintln!(
                "{} {}",
                "Done downloading input file[s] in".green(),
                fs::canonicalize(
                    inputs[0]
                        .path()
                        .parent()
                        .context("no parent folder exists")?
                )?
                .to_string_lossy()
                .yellow()
            );
        }
        yaadv::args::Commands::Credentials(creds) => {
            if creds == Default::default() {
                // default interactive mode

                let choice = inquire::Select::new(
                    "Credentials:",
                    vec![CredentialsOption::ViewToken, CredentialsOption::SetToken],
                )
                .prompt()?;

                match choice {
                    CredentialsOption::ViewToken => {
                        let token = Secrets::load();
                        match token.get_session_token() {
                            Some(token) => println!("Your session token: {}", token.bright_cyan()),
                            None => {
                                eprintln!("{}", "No session token found!".red());
                                process::exit(1);
                            }
                        }
                    }
                    CredentialsOption::SetToken => {
                        let token = inquire::Password::new("Your session token:")
                            .with_display_mode(inquire::PasswordDisplayMode::Masked)
                            .without_confirmation()
                            .prompt()?;

                        let old_token = Secrets::load();
                        if old_token.get_session_token().is_some() {
                            let confirm = inquire::Confirm::new(
                                "Your previous session token will be overwritten, continue?",
                            )
                            .with_default(false)
                            .prompt()?;
                            if !confirm {
                                process::exit(0);
                            }
                        }
                        Secrets {
                            session_token: Some(token),
                        }
                        .store()?;
                    }
                }
            }

            if let Some(token) = creds.token {
                Secrets {
                    session_token: Some(token),
                }
                .store()?;
            }

            if creds.show {
                let token = Secrets::load();
                match token.get_session_token() {
                    Some(token) => println!("Your session token: {}", token.bright_cyan()),
                    None => {
                        eprintln!("{}", "No session token found!".red());
                        process::exit(1);
                    }
                }
            }
        }
    }

    Ok(())
}
