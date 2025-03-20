use std::path::{Path, PathBuf};

use gix::{bstr::BStr, sec::trust::DefaultForLevel as _};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn open_repo(path: &Path) -> Result<gix::ThreadSafeRepository> {
    // custom open options
    let mut git_open_opts_map = gix::sec::trust::Mapping::<gix::open::Options>::default();

    // On windows various configuration options are bundled as part of the installations
    // This path depends on the install location of git and therefore requires some overhead to lookup
    // This is basically only used on windows and has some overhead hence it's disabled on other platforms.
    // `gitoxide` doesn't use this as default
    let config = gix::open::permissions::Config {
        system: true,
        git: true,
        user: true,
        env: true,
        includes: true,
        git_binary: cfg!(windows),
    };
    // change options for config permissions without touching anything else
    git_open_opts_map.reduced = git_open_opts_map
        .reduced
        .permissions(gix::open::Permissions {
            config,
            ..gix::open::Permissions::default_for_level(gix::sec::Trust::Reduced)
        });
    git_open_opts_map.full = git_open_opts_map.full.permissions(gix::open::Permissions {
        config,
        ..gix::open::Permissions::default_for_level(gix::sec::Trust::Full)
    });

    let open_options = gix::discover::upwards::Options {
        dot_git_only: true,
        ..Default::default()
    };

    let res = gix::ThreadSafeRepository::discover_with_environment_overrides_opts(
        path,
        open_options,
        git_open_opts_map,
    )?;

    Ok(res)
}

fn get_repo_dir(file: &Path) -> Result<&Path> {
    file.parent().ok_or("file has no parent directory".into())
}

pub fn get_repo(path: &Path) -> Result<gix::Repository> {
    Ok(open_repo(get_repo_dir(path)?)?.to_thread_local())
}

fn main() -> Result<()> {
    let doc = PathBuf::from("../helix-lsp-types/Cargo.toml");

    dbg!(&doc);

    let repo = get_repo(&doc)?;

    dbg!(&repo);

    let head = repo.head()?.peel_to_commit_in_place()?.id;

    let traverse = gix::traverse::commit::topo::Builder::from_iters(
        &repo.objects,
        [head],
        None::<Vec<gix::ObjectId>>,
    )
    .build()?;

    // let relative_path = doc
    //     .strip_prefix(
    //         repo.path()
    //             .parent()
    //             .ok_or("Could not get parent path of repository")?,
    //     )
    //     .unwrap_or(&doc)
    //     .to_str()
    //     .ok_or("Could not convert path to string")?;

    // dbg!(&relative_path);

    let path = PathBuf::from("helix-lsp-types/Cargo.toml");
    let mut resource_cache = repo.diff_resource_cache_for_tree_diff()?;

    let file_blame = gix::blame::file(
        &repo.objects,
        traverse.into_iter(),
        &mut resource_cache,
        BStr::new(path.to_str().unwrap()),
        None,
    )?
    .entries;

    dbg!(file_blame);

    Ok(())
}
