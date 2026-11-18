use super::*;
use std::path::{Path, PathBuf};

/// Agent memory keeps its file in the same repository folder as this
/// database, so both are found, backed up, and removed together.
#[test]
fn agent_memory_shares_the_repository_folder() {
    let state = Path::new("/state");
    let git_dir = PathBuf::from("/code/app/.git");
    let database = crate::location::database_path_in(state, &Repository::Local(git_dir.clone()));
    let memory = sheprd_memory::path_in(state, &git_dir);
    assert_eq!(memory.parent(), database.parent());
    assert_eq!(memory.file_name().unwrap(), "memory.sqlite3");
}
