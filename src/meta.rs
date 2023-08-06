#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FSFolder(pub Vec<FSMeta>);

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct FSMeta {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub created: u32,
    pub modified: u32,
    pub block_ids: Vec<u64>, // 文件块列表
}
