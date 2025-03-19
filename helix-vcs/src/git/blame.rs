use anyhow::Context as _;
use gix::bstr::BStr;
use std::{collections::HashMap, ops::Range, path::Path};

use super::{get_repo_dir, open_repo};

pub struct BlameInformation {
    pub commit_hash: String,
    pub author_name: String,
    pub author_email: String,
    pub commit_date: String,
    pub commit_message: String,
}

impl BlameInformation {
    /// Parse the user's blame format
    pub fn parse_format(&self, format: &str) -> String {
        let mut formatted = String::with_capacity(format.len() * 2);

        let variables = HashMap::from([
            ("commit", &self.commit_hash),
            ("author", &self.author_name),
            ("date", &self.commit_date),
            ("message", &self.commit_message),
            ("email", &self.author_email),
        ]);

        let mut chars = format.chars().peekable();
        while let Some(ch) = chars.next() {
            // "{{" => '{'
            if ch == '{' && chars.next_if_eq(&'{').is_some() {
                formatted.push('{');
            }
            // "}}" => '}'
            else if ch == '}' && chars.next_if_eq(&'}').is_some() {
                formatted.push('}');
            } else if ch == '{' {
                let mut variable = String::new();
                // eat all characters until the end
                while let Some(ch) = chars.next_if(|ch| *ch != '}') {
                    variable.push(ch);
                }
                // eat the '}' if it was found
                let has_closing = chars.next().is_some();
                let res = variables
                    .get(variable.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        // Invalid variable. So just add whatever we parsed before
                        let mut result = String::with_capacity(variable.len() + 2);
                        result.push('{');
                        result.push_str(variable.as_str());
                        if has_closing {
                            result.push('}');
                        }
                        result
                    });

                formatted.push_str(&res);
            } else {
                formatted.push(ch);
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

    let commit = repo.find_commit(latest_commit_id)?;
    let author = commit.author()?;

    Ok(BlameInformation {
        commit_hash: commit.short_id()?.to_string(),
        author_name: author.name.to_string(),
        author_email: author.email.to_string(),
        commit_date: author.time.format(gix::date::time::format::SHORT),
        commit_message: commit.message()?.title.to_string(),
    })
}
