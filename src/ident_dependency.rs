use cargo::ops::cargo_add::dependency::Dependency;

#[derive(Debug, PartialEq, Eq, Clone)]
#[non_exhaustive]
pub struct IdentDependency {
    pub package_name: String,
    pub dep_kind: Vec<String>,
    pub dep: Dependency,
}

impl IdentDependency {
    pub fn new(package_name: &str, dep_kind: Vec<&str>, dep: Dependency) -> Self {
        Self {
            package_name: package_name.to_string(),
            dep_kind: dep_kind.into_iter().map(String::from).collect::<Vec<_>>(),
            dep,
        }
    }
}
