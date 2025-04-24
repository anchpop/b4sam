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

fn get_changes(against: Option<&str>) -> anyhow::Result<String> {
    // Validate the against revision if provided
    if let Some(rev) = against {
        let validate = Command::new("git")
            .args(["rev-parse", "--verify", rev])
            .output();

        if !matches!(validate, Ok(ref o) if o.status.success()) {
            anyhow::bail!("Invalid git revision: {}", rev);
        }
    }

    let base = if let Some(commit) = against {
        commit.to_string()
    } else {
        // Try with origin/main first
        let mut merge_base_output = Command::new("git")
            .args(["merge-base", "origin/main", "HEAD"])
            .output();

        // If that fails, try with origin/master
        if !matches!(merge_base_output, Ok(ref o) if o.status.success()) {
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

        merge_base
    };

    // Get the diff between the base and the current HEAD
    let diff_output = Command::new("git")
        .args([
            "diff", "-U30", /* give the model 30 lines of context for the change */
            &base, "HEAD",
        ])
        .output()
        .context("Failed to run `git diff`")?;

    if !diff_output.status.success() {
        anyhow::bail!("`git diff` failed with status: {}", diff_output.status);
    }

    if diff_output.stdout.is_empty() {
        anyhow::bail!("No changes found");
    }

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
    /// Review code changes
    Review {
        /// Custom system prompt for the AI
        #[arg(short, long)]
        prompt: Option<String>,

        /// Specify a git commit to diff against (instead of using merge-base)
        #[arg(long)]
        against: Option<String>,
    },
    /// Show the diff that would be reviewed
    ShowDiff {
        /// Specify a git commit to diff against (instead of using merge-base)
        #[arg(long)]
        against: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Review { prompt, against }) => {
            review_code(prompt, cli.verbose, against.as_deref()).await?;
        }
        Some(Commands::ShowDiff { against }) => {
            let changes = get_changes(against.as_deref())?;
            println!("{}", changes);
        }
        None => {
            // Default to review if no command is specified
            review_code(None, cli.verbose, None).await?;
        }
    }

    Ok(())
}

async fn review_code(
    custom_prompt: Option<String>,
    verbose: bool,
    against: Option<&str>,
) -> anyhow::Result<()> {
    let default_prompt = r#"You are a helpful assistant that reviews code. The types of responses you can leave are "Nitpick", "LeftoverDebug", "UnnecessaryComment", "StyleIssue", "Question", "Issue", "Suggestion", "Idea". Also, redisplay the line of code that you are commenting on and tell the user where that line is in the file. Keep in mind that you will not see the entire file, only a diff that shows the sections that changed. This means that you may see variables and functions being used without seeing where they are defined. You are being invoked on code that compiles and passes all tests (you are simply a last pass sanity check).

Nitpick: Small style issues, small issues in performance (e.g. cloning a vector when passing by reference would work).
LeftoverDebug: Debug statements, println! statements, etc. that were probably left in by mistake.
UnnecessaryComment: Comments that are not needed, or explain something overly-obvious. Be very strict about this. Comments that explain what the code does are not needed. The only comments that are needed are ones provided as documentation for public parts of the API, and those that explain *why* the code is the way it is (rather than what it does).
StyleIssue: Style issues that do not fall under the other categories.
Question: Questions about the code, or questions that the user should answer before merging (e.g. have you updated the docs?).
Issue: Issues with the code that are not style related.
Suggestion: Suggestions for improvements.
Idea: Ideas for improvements.

Remember, the code you are reviewing has already been compiled without errors and passed all tests. There is no possibility that the code would not compile, and there are no errors in the code that would prevent it from compiling.
    "#;
    let system_prompt = custom_prompt.unwrap_or_else(|| default_prompt.to_string());
    let client = ChatClient::from_env("o3")?;

    if verbose {
        eprintln!("Fetching changes against default branch...");
    }

    let changes = get_changes(against)?;

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
