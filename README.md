## b4sam

This is a simple tool you can run to try to catch dumb mistakes in your pull requests before your coworkers do. It just diffs your changes against master, puts the diff into gpt-o3, then prints the output.

```
❯ b4sam
Code Review Results:
==================

[Suggestion] in: .gitignore (line 5)
           line: output/
Adding 'output/' to .gitignore is a good idea if you want to ignore generated files. Consider adding a comment above it to indicate what kind of output files are being ignored.

[Issue] in: src/bin/ingest.rs (line 30)
      line: db.find_raw_files(&storage_dir).unwrap();
The call to unwrap() on 'db.find_raw_files' may panic on error. Consider proper error handling or propagating the error.

[Issue] in: src/index/find.rs (inside the new 'look' function)
      line: match extension.to_str().unwrap().to_lowercase().as_str() {
Using unwrap() on 'to_str()' can panic if a file path is not valid UTF-8. Consider using to_string_lossy() or handling the None case.

[Suggestion] in: src/screen_control.rs (line 264)
           line: let context = futures::executor::block_on(gather_context(now));
Switching to block_on for async context gathering is acceptable in a sync context. However, if your project already uses an async runtime (like Tokio), it might be better to integrate it consistently to avoid potential runtime conflicts.
```

Why the name? I have a coworker named sam who is always catching my dumb mistakes. Now I run this b4 showing it to him to avoid the embarassment.

Obviously this is only capable of catching surface level issuse (typos, leftover debug statements, etc), and not everything it detects is a real issue, but I still find it very useful.

## Installation

```
cargo install b4sam
```

## Usage

Export the `OPENAI_API_KEY` environment variable to your OpenAI API key. (I do this in `~/.zshrc`, which is probably not ideal for security, but it is convenient haha.)

Then, simply run `b4sam` in your terminal from your branch.
