use toml_edit::Item;

#[derive(Debug, Clone)]
pub struct ItemMap {
    pub item: Item,
    pub members: Vec<String>,
}

impl ItemMap {
    pub fn new(item: Item) -> Self {
        Self {
            item,
            members: vec![],
        }
    }

    pub fn add_member(&mut self, member_name: String) {
        self.members.push(member_name);
    }
}
