use anyhow::Context as _;
use gix::bstr::BStr;
use std::{collections::HashMap, ops::Range, path::Path};

use super::{get_repo_dir, open_repo};

pub struct BlameInformation {
    pub commit_hash: Option<String>,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub commit_date: Option<String>,
    pub commit_message: Option<String>,
    pub commit_body: Option<String>,
}

impl BlameInformation {
    /// Parse the user's blame format
    pub fn parse_format(&self, format: &str) -> String {
        let mut formatted = String::new();
        let mut content_before_variable = String::new();

        let variables = HashMap::from([
            ("commit", &self.commit_hash),
            ("author", &self.author_name),
            ("date", &self.commit_date),
            ("message", &self.commit_message),
            ("email", &self.author_email),
            ("body", &self.commit_body),
        ]);

        let mut chars = format.chars().peekable();
        // in all cases, when any of the variables is empty we exclude the content before the variable
        // However, if the variable is the first and it is empty - then exclude the content after the variable
        let mut exclude_content_after_variable = false;
        while let Some(ch) = chars.next() {
            // "{{" => '{'
            if ch == '{' && chars.next_if_eq(&'{').is_some() {
                content_before_variable.push('{');
            }
            // "}}" => '}'
            else if ch == '}' && chars.next_if_eq(&'}').is_some() {
                content_before_variable.push('}');
            } else if ch == '{' {
                let mut variable = String::new();
                // eat all characters until the end
                while let Some(ch) = chars.next_if(|ch| *ch != '}') {
                    variable.push(ch);
                }
                // eat the '}' if it was found
                let has_closing = chars.next().is_some();

                #[derive(PartialEq, Eq, PartialOrd, Ord)]
                enum Variable {
                    Valid(String),
                    Invalid(String),
                    Empty,
                }

                let variable_value = variables.get(variable.as_str()).map_or_else(
                    || {
                        // Invalid variable. So just add whatever we parsed before
                        let mut result = String::with_capacity(variable.len() + 2);
                        result.push('{');
                        result.push_str(variable.as_str());
                        if has_closing {
                            result.push('}');
                        }
                        Variable::Invalid(result)
                    },
                    |s| {
                        s.as_ref()
                            .map(|s| Variable::Valid(s.to_string()))
                            .unwrap_or(Variable::Empty)
                    },
                );

                match variable_value {
                    Variable::Valid(value) => {
                        if exclude_content_after_variable {
                            // don't push anything.
                            exclude_content_after_variable = false;
                        } else {
                            formatted.push_str(&content_before_variable);
                        }
                        formatted.push_str(&value);
                    }
                    Variable::Invalid(value) => {
                        if exclude_content_after_variable {
                            // don't push anything.
                            exclude_content_after_variable = false;
                        } else {
                            formatted.push_str(&content_before_variable);
                        }
                        formatted.push_str(&value);
                    }
                    Variable::Empty => {
                        if formatted.is_empty() {
                            // exclude content AFTER this variable (at next iteration of the loop,
                            // we'll exclude the content before a valid variable)
                            exclude_content_after_variable = true;
                        } else {
                            // exclude content BEFORE this variable
                            // also just don't add anything.
                        }
                    }
                }

                content_before_variable.drain(..);
            } else {
                content_before_variable.push(ch);
            }
        }

        formatted
    }
}

/// `git blame` a range in a file
pub fn blame(
    file: &Path,
    range: Range<u32>,
    added_lines_count: u32,
    removed_lines_count: u32,
) -> anyhow::Result<BlameInformation> {
    // Because gix_blame doesn't care about stuff that is not commited, we have to "normalize" the
    // line number to account for uncommited code.
    //
    // You'll notice that blame_line can be 0 when, for instance we have:
    // - removed 0 lines
    // - added 10 lines
    // - cursor_line is 8
    //
    // So when our cursor is on the 10th added line or earlier, blame_line will be 0. This means
    // the blame will be incorrect. But that's fine, because when the cursor_line is on some hunk,
    // we can show to the user nothing at all
    let normalize = |line: u32| line.saturating_sub(added_lines_count) + removed_lines_count;

    let blame_range = normalize(range.start)..normalize(range.end);

    let repo_dir = get_repo_dir(file)?;
    let repo = open_repo(repo_dir)
        .context("failed to open git repo")?
        .to_thread_local();

    let suspect = repo.head()?.peel_to_commit_in_place()?;

    let relative_path = file
        .strip_prefix(
            repo.path()
                .parent()
                .context("Could not get parent path of repository")?,
        )
        .unwrap_or(file)
        .to_str()
        .context("Could not convert path to string")?;

    let traverse_all_commits = gix::traverse::commit::topo::Builder::from_iters(
        &repo.objects,
        [suspect.id],
        None::<Vec<gix::ObjectId>>,
    )
    .build()?;

    let mut resource_cache = repo.diff_resource_cache_for_tree_diff()?;
    let latest_commit_id = gix::blame::file(
        &repo.objects,
        traverse_all_commits,
        &mut resource_cache,
        BStr::new(relative_path),
        Some(blame_range),
    )?
    .entries
    .first()
    .context("No commits found")?
    .commit_id;

    let commit = repo.find_commit(latest_commit_id).ok();
    let message = commit.as_ref().and_then(|c| c.message().ok());
    let author = commit.as_ref().and_then(|c| c.author().ok());

    Ok(BlameInformation {
        commit_hash: commit
            .as_ref()
            .and_then(|c| c.short_id().map(|id| id.to_string()).ok()),
        author_name: author.map(|a| a.name.to_string()),
        author_email: author.map(|a| a.email.to_string()),
        commit_date: author.map(|a| a.time.format(gix::date::time::format::SHORT)),
        commit_message: message.as_ref().map(|msg| msg.title.to_string()),
        commit_body: message
            .as_ref()
            .and_then(|msg| msg.body.map(|body| body.to_string())),
    })
}
