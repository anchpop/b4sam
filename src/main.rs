use std::process::Command;

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

fn get_changes_against_master() -> String {
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

    let merge_base_output = merge_base_output.expect("Failed to run git merge-base");
    let merge_base = String::from_utf8_lossy(&merge_base_output.stdout)
        .trim()
        .to_string();

    if merge_base.is_empty() {
        return String::from("Failed to find merge base with origin/main or origin/master");
    }

    // Get the diff between the merge base and the current HEAD
    let diff_output = Command::new("git")
        .args(["diff", &merge_base])
        .output()
        .expect("Failed to run git diff");

    String::from_utf8_lossy(&diff_output.stdout).to_string()
}

#[tokio::main]
async fn main() {
    let system_prompt = r#"You are a helpful assistant that reviews code. The types of responses you can leave are "Nitpick", "LeftoverDebug", "UnnecessaryComment", "StyleIssue", "Question", "Issue", "Suggestion", "Idea". Also, redisplay the line of code that you are commenting on and tell the user where that line is in the file."#;
    let client = ChatClient::from_env("o3-mini").unwrap();

    let changes = get_changes_against_master();
    let review: Review = client
        .chat_with_system_prompt(&changes, system_prompt)
        .await
        .unwrap();
    
    // Display usage information
    let usage = client.usage.read().unwrap();
    
    // Cost calculation (USD per million tokens)
    const INPUT_COST_PER_MILLION: f64 = 1.10;
    const OUTPUT_COST_PER_MILLION: f64 = 4.40;
    
    let input_cost = (usage.prompt_tokens as f64 / 1_000_000.0) * INPUT_COST_PER_MILLION;
    let output_cost = (usage.completion_tokens as f64 / 1_000_000.0) * OUTPUT_COST_PER_MILLION;
    let total_cost = input_cost + output_cost;
    
    println!("📊 Token Usage & Cost Summary");
    println!("-----------------------------");
    println!("Total:  {:5} tokens (${:.4})", usage.total_tokens, total_cost);
    println!();
    
    println!("Code Review Results:");
    println!("==================\n");

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
}
