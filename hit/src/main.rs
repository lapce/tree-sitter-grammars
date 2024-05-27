use std::path::Path;

use anyhow::{anyhow, Result};
use git2::{ConfigEntry, FetchOptions, RemoteCallbacks, Repository, SubmoduleUpdateOptions};
use indexmap::{IndexMap, IndexSet};

#[derive(Debug, Clone, Default)]
struct Module {
    _name: String,
    path: String,
    branch: String,
    url: IndexSet<String>,
}

fn set_val(submod: &mut Module, key: &str, entry: &ConfigEntry) {
    match key {
        "branch" => {
            submod.branch = entry.value().unwrap().to_string();
        }
        "path" => {
            submod.path = entry.value().unwrap().to_string();
        }
        "url" => {
            submod.url.insert(entry.value().unwrap().to_string());
        }
        _ => {
            eprintln!("Unknown entry type: {key}")
        }
    }
}

fn main() -> Result<()> {
    let arg1 = std::env::args().nth(1);

    let cfg = git2::Config::open(Path::new(".gitmodules"))?;

    let mut config: IndexMap<String, Module> = IndexMap::new();

    let mut entries = cfg.entries(None)?;
    while let Some(entry) = entries.next() {
        let entry = entry?;

        // println!("{} => {}", entry.name().unwrap(), entry.value().unwrap());

        let mut entry_key = entry.name().unwrap().split('.');
        let _entry_type = entry_key.next().unwrap();
        let entry_name = entry_key.next().unwrap();
        let entry_key = entry_key.next().unwrap();

        match config.get_mut(entry_name) {
            Some(submod) => {
                set_val(submod, entry_key, entry);
            }
            None => {
                let mut submod = Module::default();

                set_val(&mut submod, entry_key, entry);

                config.insert(entry_name.to_string(), submod);
            }
        }
    }

    config.sort_unstable_keys();

    match arg1.as_deref() {
        Some("print") => {
            for (name, submod) in config {
                println!("[submodule \"{name}\"]");
                if !submod.branch.is_empty() {
                    println!("\tbranch = \"{}\"", submod.branch);
                }
                if !submod.path.is_empty() {
                    println!("\tpath = \"{}\"", submod.path);
                }
                if !submod.url.is_empty() {
                    for url in submod.url {
                        println!("\turl = \"{url}\"");
                    }
                }
                println!();
            }
        }

        Some("update") => {
            let repo = git2::Repository::open(".")?;

            let modules = repo.submodules()?;

            for mut module in modules {
                eprintln!("Submodule: {}", module.name().unwrap());

                if let Some(submod) = config.get(module.name().unwrap()) {
                    let module = match module.open() {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("\tFailed to open module: {e}");
                            if e.code() == git2::ErrorCode::NotFound {
                                if let Err(e) = module.repo_init(false) {
                                    eprintln!("\tFailed to repo init module: {e}");
                                };

                                let mut cb = RemoteCallbacks::new();
                                cb.credentials(
                                    |_url, username_from_url: Option<&str>, _allowed_types| {
                                        git2::Cred::ssh_key_from_agent(username_from_url.unwrap())
                                    },
                                );

                                let mut fo = FetchOptions::new();
                                fo.remote_callbacks(cb);

                                if let Err(e) =
                                    module.clone(Some(SubmoduleUpdateOptions::new().fetch(fo)))
                                {
                                    eprintln!("Failed to update module: {e}");
                                };

                                match module.open() {
                                    Ok(v) => v,
                                    Err(e) => {
                                        return Err(anyhow!("Failed to open module AGAIN: {e}"));
                                    }
                                }
                            } else {
                                panic!("\tDiff err")
                            }
                        }
                    };

                    // Create remotes for all URLs defined for submodule
                    // First URL should be that of an upstream and will be created additionally under that name
                    // Last URL should be an origin, which will be created for that name
                    for i in 0..submod.url.len() {
                        let remote = submod.url.get_index(i).unwrap();
                        let remote_name = get_remote_name_from_url(remote);
                        create_remote(&module, remote_name, remote)?;
                        if i == 0 {
                            create_remote(&module, "upstream", remote)?;
                        }
                        if i == submod.url.len() {
                            create_remote(&module, "origin", remote)?;
                        }
                    }
                }
            }
        }

        v => {
            eprintln!("Unknown action: {v:?}");
        }
    }

    Ok(())
}

fn get_remote_name_from_url(url: &str) -> &str {
    let mut url = url.split(':');
    url.next();
    url.next().unwrap().split('/').take(1).next().unwrap()
}

fn create_remote(module: &Repository, name: &str, url: &str) -> Result<()> {
    match module.find_remote(name) {
        Ok(r) => {
            if r.url().unwrap_or_default() != url {
                if let Err(e) = module.remote_set_url(name, url) {
                    eprintln!("\tFailed to update remote `{name}`: {e}");
                } else {
                    eprintln!("\tRemote `{name}` updated.");
                };
            } else {
                eprintln!("\tRemote `{name}` found.");
            }
            return Ok(());
        }
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                eprintln!("\tRemote `{name}` not found. Creating...");
                if let Err(e) = module.remote(name, url) {
                    eprintln!("\tFailed to create remote `{name}`: {e}");
                };
            } else {
                eprintln!("\tFailed to find remote `{name}`: {e}");
            }
        }
    };
    Ok(())
}
