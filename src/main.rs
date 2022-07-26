mod ident_dependency;
mod item_map;

use crate::ident_dependency::IdentDependency;
use crate::item_map::ItemMap;
use anyhow::{anyhow, bail, Context};
use cargo::core::Workspace;
use cargo::ops::cargo_add::dependency::{PathSource, Source, WorkspaceSource};
use cargo::ops::cargo_add::{dependency::Dependency, manifest::LocalManifest};
use cargo::CargoResult;
use indexmap::IndexSet;
use itertools::Itertools;
use semver::{BuildMetadata, Comparator, Prerelease, Version};
use std::collections::{BTreeMap, HashMap};
use toml_edit::Item;

// Do not try and merge readme, or license-file
const PKG_KEYS: [&str; 14] = [
    "authors",
    "categories",
    "description",
    "documentation",
    "edition",
    "exclude",
    "homepage",
    "include",
    "keywords",
    "license",
    "publish",
    "repository",
    "rust-version",
    "version",
];

fn main() -> CargoResult<()> {
    let config = cargo::Config::default()?;
    let path = cargo::util::important_paths::find_root_manifest_for_wd(config.cwd())?;
    let ws = Workspace::new(&path, &config)?;
    let mut manifest = LocalManifest::try_new(&path)?;
    let ws_name = if !ws.is_virtual() {
        manifest.clone().package_name()?.to_string()
    } else {
        String::new()
    };

    let mut members = find_members(&ws)?;
    let mut all_deps: HashMap<String, Vec<IdentDependency>> = HashMap::new();
    let mut all_pkg_keys: BTreeMap<&str, HashMap<String, ItemMap>> = BTreeMap::new();

    for (member_name, member) in &members {
        let project = member
            .get_table(&[String::from("package")])
            .unwrap()
            .as_table_like()
            .unwrap();
        for key in PKG_KEYS {
            if let Some(item) = project.get(key) {
                let entry = all_pkg_keys.entry(key).or_insert_with(HashMap::new);
                let inner = entry
                    .entry(item.to_string())
                    .or_insert_with(|| ItemMap::new(item.clone()));
                inner.add_member(member_name.to_string());
            }
        }
        for (dep_table, table) in member.manifest.get_sections() {
            table
                .as_table_like()
                .unwrap()
                .iter()
                .for_each(|(name, item)| {
                    let dep =
                        Dependency::from_toml(member.path.parent().unwrap(), name, item).unwrap();
                    let ident_dep = IdentDependency::new(member_name, dep_table.to_table(), dep);
                    let entry = all_deps
                        .entry(ident_dep.dep.toml_key().to_string())
                        .or_insert(vec![]);
                    entry.push(ident_dep);
                });
        }
    }

    // Process dependencies first so `[workspace.dependencies]` is first in a `Cargo.toml`
    let shared_deps = all_deps
        .into_iter()
        .filter(|(_, deps)| deps.len() > 1)
        .map(|(name, deps)| (create_ws_dep(&name, &deps).unwrap(), deps))
        .sorted_by(|a, b| a.0.toml_key().cmp(b.0.toml_key()))
        .collect::<Vec<(Dependency, Vec<IdentDependency>)>>();

    let ws_dep_table = vec!["workspace", "dependencies"];
    let ws_dep_table = ws_dep_table
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

    for (ws_dep, ident_deps) in shared_deps {
        if !ws.is_virtual() {
            let member = members.get_mut(&ws_name).unwrap();
            member.insert_into_table(&ws_dep_table, &ws_dep)?;
        } else {
            manifest.insert_into_table(&ws_dep_table, &ws_dep)?;
        }

        for ident_dep in ident_deps {
            let mut dep = ident_dep.dep;

            if let Some(false) = ws_dep.default_features {
                if dep.default_features().unwrap_or(true) {
                    dep = dep.extend_features(vec![String::from("default")]);
                }
            }

            dep = dep.set_source(WorkspaceSource::new());
            if let Some(inherit_feats) = ws_dep.features.clone() {
                dep.inherited_features = Some(inherit_feats);
            }

            if let Some(features) = &dep.features {
                if features.is_empty() {
                    dep.features = None;
                }
            }
            let member = members.get_mut(&ident_dep.package_name).unwrap();
            member.insert_into_table(&ident_dep.dep_kind, &dep)?;
        }
    }

    // Process dependencies first so `[workspace.package]` is second in a `Cargo.toml`
    let ws_pkg_table = vec!["workspace", "package"];
    let ws_pkg_table = ws_pkg_table
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

    let mut ws_item = toml_edit::InlineTable::default();
    ws_item.set_dotted(true);
    ws_item.insert("workspace", true.into());
    let ws_item = toml_edit::value(toml_edit::Value::InlineTable(ws_item));

    for (key, value) in all_pkg_keys {
        if value.len() == 1 {
            let item_map = &value.values().take(1).next().unwrap();
            if item_map.members.len() > 1 {
                if !ws.is_virtual() {
                    let member = members.get_mut(&ws_name).unwrap();
                    write_pkg_key(member, &ws_pkg_table, key, &item_map.item)?;
                } else {
                    write_pkg_key(&mut manifest, &ws_pkg_table, key, &item_map.item)?;
                }

                for member_name in &item_map.members {
                    let member = members.get_mut(member_name).unwrap();
                    write_pkg_key(member, &[String::from("package")], key, &ws_item)?;
                }
            }
        } else {
            println!(
                "There were conflicting values for {:?}, not adding to [workspace.package]",
                key
            );
        }
    }

    for (name, mut member) in members {
        if name == ws_name {
            sort_ws_inherit_tables(&mut member)
        }
        member.write()?;
    }

    // Only write the main manifest if it is virtual
    // This is because if it is a "member" it will be written above
    if ws.is_virtual() {
        manifest.write()?;
    }

    Ok(())
}

/// Find all members in the workspace
fn find_members(ws: &Workspace) -> CargoResult<HashMap<String, LocalManifest>> {
    let mut members: HashMap<String, LocalManifest> = HashMap::new();
    for member in ws.members() {
        let member_name = member.name();
        let path = member.manifest_path();
        let member = LocalManifest::try_new(path)?;
        members.insert(member_name.to_string(), member);
    }
    Ok(members)
}

/// Create a Dependency to be added to `[workspace.dependencies]`
fn create_ws_dep(name: &str, deps: &[IdentDependency]) -> CargoResult<Dependency> {
    let mut source: IndexSet<Source> = IndexSet::new();
    let mut features: IndexSet<String> = IndexSet::new();
    let mut versions: Vec<&str> = Vec::new();
    let mut default_features: Option<bool> = None;

    for (i, dep) in deps.iter().enumerate() {
        if source.is_empty() {
            // Modify the path to be relative to the workspace root
            let inner = if let Some(Source::Path(path_source)) = &dep.dep.source {
                let path = cargo_util::paths::normalize_path(&path_source.path);
                Source::Path(PathSource::new(path))
            } else {
                dep.dep.source.clone().unwrap()
            };
            source.insert(inner);
        }

        // Collect all versions to pick a compatible semver
        if let Some(dep_ver) = dep.dep.version() {
            versions.push(dep_ver);
        }

        // Only set to false if one of the deps do
        if let Some(false) = dep.dep.default_features {
            default_features = Some(false)
        }

        // Make sure to get the smallest amount of features to share
        if i == 0 {
            if let Some(feat) = &dep.dep.features {
                features = feat.clone();
            }
        } else if let Some(feat) = &dep.dep.features {
            let temp = features.clone();
            features.clear();
            temp.intersection(feat).for_each(|f| {
                features.insert(f.clone());
            });
        } else {
            features.clear();
        }
    }

    let mut dep = Dependency::new(name);
    let version = if versions.is_empty() {
        None
    } else {
        Some(select_semver(&versions).context(format!("Failed to select semver for {}", name))?)
    };
    let dep_source: Source = match source.first().unwrap().clone() {
        Source::Registry(mut reg) => {
            reg.version = version
                .ok_or_else(|| anyhow!("a registry source requires a version"))?
                .to_string();
            Source::Registry(reg)
        }
        Source::Path(mut path) => {
            path.version = version.map(|ver| ver.to_string());
            Source::Path(path)
        }
        Source::Git(mut git) => {
            git.version = version.map(|ver| ver.to_string());
            Source::Git(git)
        }
        Source::Workspace(ws) => Source::Workspace(ws),
    };

    dep = dep.set_source(dep_source);

    if default_features.is_some() {
        dep = dep.set_default_features(false)?
    }

    if !features.is_empty() {
        dep = dep.set_features(features)
    }
    Ok(dep)
}

/// This selects the newest semver that is compatible with all deps
fn select_semver<'a>(versions: &[&'a str]) -> CargoResult<&'a str> {
    let mut semver: Option<&str> = None;
    for new in versions {
        if let Some(version) = semver {
            semver = Some(compare_versions(version, new)?);
        } else {
            semver = Some(new);
        }
    }
    Ok(semver.unwrap())
}

/// Compares two semver and chooses the newer one if it is compatible
fn compare_versions<'a>(base: &'a str, new: &'a str) -> CargoResult<&'a str> {
    let base_comp = Comparator::parse(base)?;
    let new_comp = Comparator::parse(new)?;
    match (
        base_comp.matches(&comp_to_ver(&new_comp)),
        new_comp.matches(&comp_to_ver(&base_comp)),
    ) {
        (true, true) => Ok(base),
        (true, false) => Ok(new),
        (false, true) => Ok(base),
        (false, false) => bail!("{} and {} are not compatible", base, new),
    }
}

fn comp_to_ver(comp: &Comparator) -> Version {
    Version {
        major: comp.major,
        minor: comp.minor.unwrap_or(0),
        patch: comp.patch.unwrap_or(0),
        pre: Prerelease::EMPTY,
        build: BuildMetadata::EMPTY,
    }
}

fn write_pkg_key(
    local: &mut LocalManifest,
    table_path: &[String],
    key: &str,
    item: &Item,
) -> CargoResult<()> {
    let table = local.get_table_mut(table_path)?;
    if let Some((mut key, ex_item)) = table.as_table_like_mut().unwrap().get_key_value_mut(key) {
        *ex_item = item.clone();
        key.fmt();
    } else {
        table[key] = item.clone();
    }

    if let Some(t) = table.as_inline_table_mut() {
        t.fmt()
    }
    Ok(())
}

fn sort_ws_inherit_tables(member: &mut LocalManifest) {
    let ws_dep_table = vec!["workspace", "dependencies"];
    let ws_dep_table = ws_dep_table
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    if let Ok(dep_table) = member.get_table_mut(&ws_dep_table) {
        dep_table.as_table_like_mut().unwrap().sort_values();
    }

    let ws_pkg_table = vec!["workspace", "package"];
    let ws_pkg_table = ws_pkg_table
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    if let Ok(pkg_table) = member.get_table_mut(&ws_pkg_table) {
        pkg_table.as_table_like_mut().unwrap().sort_values();
    }
}

#[cfg(test)]
mod tests {
    use crate::select_semver;

    #[test]
    fn semver_minor_patch() {
        let versions = vec!["0.3.2", "0.3", "0.3.10"];
        assert_eq!("0.3.10", select_semver(&versions).unwrap());

        let versions = vec!["0.3.2", "0.4", "0.3.10"];
        select_semver(&versions).expect_err("Expected error");
    }

    #[test]
    fn semver_major_minor_patch() {
        let versions = vec!["1.3.2", "1.3", "1.3.10"];
        assert_eq!("1.3.10", select_semver(&versions).unwrap());

        let versions = vec!["1.3.2", "1.4", "1.3.10"];
        assert_eq!("1.4", select_semver(&versions).unwrap());

        let versions = vec!["2.3.2", "1.4", "1.3.10"];
        select_semver(&versions).expect_err("Expected error");
    }
}
