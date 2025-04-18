use std::process::Command;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tysm::chat_completions::ChatClient;

#[derive(serde::Deserialize, schemars::JsonSchema, Debug)]
enum CommentType {
    Nitpick,
    LeftoverDebug,
    UnnecessaryComment,
    StyleIssue,
    Question,
    Issue,
    Suggestion,
    Idea,
}

impl std::fmt::Display for CommentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommentType::Nitpick => write!(f, "Nitpick"),
            CommentType::LeftoverDebug => write!(f, "LeftoverDebug"),
            CommentType::UnnecessaryComment => write!(f, "UnnecessaryComment"),
            CommentType::StyleIssue => write!(f, "StyleIssue"),
            CommentType::Question => write!(f, "Question"),
            CommentType::Issue => write!(f, "Issue"),
            CommentType::Suggestion => write!(f, "Suggestion"),
            CommentType::Idea => write!(f, "Idea"),
        }
    }
}

#[derive(serde::Deserialize, schemars::JsonSchema, Debug)]
struct Comment {
    comment_type: CommentType,
    r#in: String,
    line: String,
    comment: String,
}

#[derive(serde::Deserialize, schemars::JsonSchema, Debug)]
struct Review {
    comments: Vec<Comment>,
}

fn get_changes_against_default_branch() -> anyhow::Result<String> {
    // Try with origin/main first
    let mut merge_base_output = Command::new("git")
        .args(["merge-base", "origin/main", "HEAD"])
        .output();

    // If that fails, try with origin/master
    if merge_base_output.is_err() || merge_base_output.as_ref().unwrap().status.code() != Some(0) {
        merge_base_output = Command::new("git")
            .args(["merge-base", "origin/master", "HEAD"])
            .output();
    }

    let merge_base_output = merge_base_output.context("Failed to run `git merge-base`")?;
    let merge_base = String::from_utf8_lossy(&merge_base_output.stdout)
        .trim()
        .to_string();

    if merge_base.is_empty() {
        anyhow::bail!("Failed to find merge base with origin/main or origin/master");
    }

    // Get the diff between the merge base and the current HEAD
    let diff_output = Command::new("git")
        .args([
            "diff",
            "-U30", /* give the model 30 lines of context for the change */
            &merge_base,
        ])
        .output()
        .context("Failed to run `git diff`")?;

    Ok(String::from_utf8_lossy(&diff_output.stdout).to_string())
}

/// CLI tool for AI-powered code reviews
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Review code changes against the default branch
    Review {
        /// Custom system prompt for the AI
        #[arg(short, long)]
        prompt: Option<String>,
    },
    /// Show the diff that would be reviewed
    ShowDiff,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Review { prompt }) => {
            review_code(prompt, cli.verbose).await?;
        }
        Some(Commands::ShowDiff) => {
            let changes = get_changes_against_default_branch()?;
            println!("{}", changes);
        }
        None => {
            // Default to review if no command is specified
            review_code(None, cli.verbose).await?;
        }
    }

    Ok(())
}

async fn review_code(custom_prompt: Option<String>, verbose: bool) -> anyhow::Result<()> {
    let default_prompt = r#"You are a helpful assistant that reviews code. The types of responses you can leave are "Nitpick", "LeftoverDebug", "UnnecessaryComment", "StyleIssue", "Question", "Issue", "Suggestion", "Idea". Also, redisplay the line of code that you are commenting on and tell the user where that line is in the file."#;

    let system_prompt = custom_prompt.unwrap_or_else(|| default_prompt.to_string());
    let client = ChatClient::from_env("o3")?;

    if verbose {
        eprintln!("Fetching changes against default branch...");
    }

    let changes = get_changes_against_default_branch()?;

    if verbose {
        eprintln!("Sending changes to AI for review...");
    }

    let review: Review = client
        .chat_with_system_prompt(&system_prompt, &changes)
        .await?;

    // Display usage information
    let cost = client.cost().unwrap_or(0.0);

    println!("Code Review Results [${:.2}]", cost);
    println!("===================\n");

    for comment in review.comments {
        let color = match comment.comment_type {
            CommentType::Nitpick => "\x1b[38;5;208m",          // Orange
            CommentType::LeftoverDebug => "\x1b[38;5;9m",      // Bright Red
            CommentType::UnnecessaryComment => "\x1b[38;5;8m", // Gray
            CommentType::StyleIssue => "\x1b[38;5;226m",       // Yellow
            CommentType::Question => "\x1b[38;5;39m",          // Blue
            CommentType::Issue => "\x1b[38;5;196m",            // Red
            CommentType::Suggestion => "\x1b[38;5;34m",        // Green
            CommentType::Idea => "\x1b[38;5;141m",             // Purple
        };
        let reset = "\x1b[0m";

        let comment_type = format!("{}", comment.comment_type);
        println!("{}[{}]{} in: {}", color, comment_type, reset, comment.r#in);
        println!(
            "{}line: {}",
            " ".repeat(comment_type.len() + 1),
            comment.line.trim()
        );
        println!("{}{}{}\n", color, comment.comment, reset);
    }

    Ok(())
}
